// Bootstrap Krello : shell, router, WS, raccourcis clavier.

import { api } from './api.js';
import { escapeHtml, priorityBadge } from './format.js';
import { startWs, on, emit } from './ws.js';
import { toast } from './ui/modal.js';
import { state, applyTheme, applyStyle, applyBoardBackground, loadBoards, loadBoard, hydrateLabelIds, initUiSettings } from './state.js';
import { startRouter, boardUrl, navigate } from './router.js';
import { initPalette } from './ui/palette.js';
import { initTopbarShortcuts, renderTopbar } from './components/topbar.js';
import { initSidebar, renderSidebar } from './components/sidebar.js';
import { renderBoard } from './views/board.js';
import { renderCalendar } from './views/calendar.js';
import { renderTimeline } from './views/timeline.js';
import { renderTable } from './views/table.js';
import { renderDashboard } from './views/dashboard.js';
import { renderSettings } from './views/settings.js';
import { initDeepLink } from './deeplink.js';
import { loadViewSettings } from './views/view_helpers.js';
import { icon } from './icons.js';

const app = document.getElementById('app');
applyTheme();
applyStyle();
initUiSettings().then(() => {
  const tb = document.getElementById('topbar');
  if (tb) renderTopbar(tb);
  const sb = document.getElementById('sidebar');
  if (sb) renderSidebar(sb);
}).catch(() => {});

// Statut IA asynchrone (non bloquant au boot) : pilote l'affichage des boutons ✨.
state.aiStatus = { available: false, reason: 'loading' };
api.aiStatus()
  .then((res) => {
    state.aiStatus = res;
    const tb = document.getElementById('topbar');
    if (tb) renderTopbar(tb);
  })
  .catch(() => {
    state.aiStatus = { available: false, reason: 'unreachable' };
  });

// Infos système (plateforme, binaire courant) — pilote l'onglet macOS.
api.system()
  .then((s) => {
    state.system = s;
  })
  .catch(() => {
    state.system = null;
  });

function shellHtml() {
  const isCollapsed = localStorage.getItem('krello-sidebar-collapsed') === 'true';
  return `
    <div class="topbar" id="topbar"></div>
    <div class="shell">
      <div class="sidebar ${isCollapsed ? 'collapsed' : ''}" id="sidebar"></div>
      <div class="main" id="main"><div class="view"></div></div>
    </div>`;
}

let currentBoardId = null;
let currentView = null;

// Accueil « Vue d'ensemble » : stats cross-board, tableau par tableau et
// activité récente. Re-rendu à chaque événement WS via renderRoute.
const ACTIVITY_FR = {
  card_created: 'Carte créée',
  card_updated: 'Carte modifiée',
  card_moved: 'Carte déplacée',
  card_duplicated: 'Carte dupliquée',
  card_deleted: 'Carte supprimée',
  card_restored: 'Carte restaurée',
  recurrence_next: 'Récurrence → occurrence suivante',
  recurrence_error: 'Erreur de récurrence',
  comment_added: 'Commentaire ajouté',
  attachment_added: 'Pièce jointe ajoutée',
  attachment_deleted: 'Pièce jointe supprimée',
  label_attached: 'Label ajouté',
  label_detached: 'Label retiré',
  checklist_created: 'Checklist créée',
  field_set: 'Champ personnalisé modifié',
  dependency_added: 'Dépendance ajoutée',
  dependency_removed: 'Dépendance retirée',
  baseline_created: 'Baseline créée',
  baseline_deleted: 'Baseline supprimée',
  board_created: 'Tableau créé',
  board_updated: 'Tableau modifié',
  board_deleted: 'Tableau supprimé',
  list_created: 'Liste créée',
  list_deleted: 'Liste supprimée',
  reminder_due: 'Rappel déclenché',
};
const activityLabel = (k) => ACTIVITY_FR[k] || k;

// Jour ISO local (sans UTC shift) : "YYYY-MM-DD".
const isoDay = (offset = 0) => {
  const d = new Date(Date.now() + offset * 86400000);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
};
const dayLabel = (due) => {
  const d = String(due || '').slice(0, 10);
  if (d === isoDay(0)) return "Aujourd'hui";
  if (d === isoDay(1)) return 'Demain';
  if (d === isoDay(2)) return 'Après-demain';
  return new Date(`${d}T12:00:00`).toLocaleDateString('fr-FR', { day: 'numeric', month: 'short' });
};

async function renderHomeOverview(main) {
  main.innerHTML = `
    <div class="view view-enter">
      <div style="padding:28px 32px;max-width:1080px;margin:0 auto">
        <h2 style="margin:0 0 18px;font-size:20px;font-weight:700">Vue d'ensemble</h2>
        <div id="home-stats" class="muted" style="font-size:13px">Chargement…</div>
      </div>
    </div>`;
  let st;
  try {
    st = await api.globalStats();
  } catch (e) {
    const box = main.querySelector('#home-stats');
    if (box) box.textContent = `Statistiques indisponibles : ${e.message}`;
    return;
  }
  const tile = (label, value, color = 'var(--text-primary)') => `
    <div style="flex:1;min-width:150px;background:var(--bg-surface);border:1px solid var(--border-subtle);border-radius:14px;padding:16px 18px">
      <div style="font-size:26px;font-weight:800;color:${color}">${value}</div>
      <div class="muted" style="font-size:12.5px;margin-top:2px">${label}</div>
    </div>`;
  const rows = (st.per_board || []).map((b) => `
    <button class="btn ghost sm" data-open-board="${escapeHtml(b.id)}" style="justify-content:space-between;width:100%;text-align:left">
      <span>${escapeHtml(b.title)}</span>
      <span class="muted" style="font-size:12px">${b.active} carte(s)${b.overdue ? ` · <b style="color:var(--error)">${b.overdue} en retard</b>` : ''}</span>
    </button>`).join('');
  const acts = (st.activity || []).map((a) => `
    <div style="display:flex;gap:8px;align-items:baseline;font-size:12.5px;padding:4px 0;border-bottom:1px solid var(--border-subtle)">
      <span class="muted" style="flex:none">${escapeHtml(String(a.created_at || '').slice(0, 16).replace('T', ' '))}</span>
      <span>${escapeHtml(activityLabel(a.kind))}</span>
    </div>`).join('');
  const boardTitle = (bid) => (state.boards || []).find((b) => b.id === bid)?.title || '—';
  const cardRow = (c, badge) => `
    <button class="btn ghost sm" data-open-card="${escapeHtml(c.board_id)}|${escapeHtml(c.id)}" style="justify-content:space-between;width:100%;text-align:left;gap:8px">
      <span style="overflow:hidden;text-overflow:ellipsis;white-space:nowrap">${escapeHtml(c.title)}</span>
      <span style="flex:none;display:flex;gap:6px;align-items:center">${badge}<span class="muted" style="font-size:11.5px">${escapeHtml(boardTitle(c.board_id))}</span></span>
    </button>`;
  const more = (shown, total) =>
    total > shown ? `<span class="muted" style="font-size:12px;padding:2px 4px">+ ${total - shown} autre(s)…</span>` : '';
  const urgentList = (st.urgent || []).map((c) => cardRow(c, priorityBadge(c))).join('')
    || '<span class="muted" style="font-size:12.5px">Rien d\'urgent 🎉</span>';
  const dueSoonList = (st.due_soon || []).map((c) => cardRow(c, `<span class="badge" style="color:var(--warning);background:var(--warning-soft);border-color:color-mix(in srgb,var(--warning)35%,transparent)">${escapeHtml(dayLabel(c.due))}</span>`)).join('')
    || '<span class="muted" style="font-size:12.5px">Aucune échéance proche</span>';
  const box = main.querySelector('#home-stats');
  if (!box) return;
  box.innerHTML = `
    <div style="display:flex;gap:14px;flex-wrap:wrap;margin-bottom:22px">
      ${tile('Tableaux', st.boards)}
      ${tile('Cartes actives', st.cards_active)}
      ${tile('En retard', st.overdue, st.overdue ? 'var(--error)' : 'var(--text-primary)')}
      ${tile('À venir (7 j)', st.due_7d, 'var(--warning)')}
      ${tile('Terminées (7 j)', st.done_7d, 'var(--success)')}
    </div>
    <div style="display:grid;grid-template-columns:1fr 1fr;gap:18px;margin-bottom:22px">
      <div>
        <h3 style="margin:0 0 8px;font-size:14px">🔥 Urgentes <span class="muted" style="font-weight:400;font-size:12px">(${st.urgent_total || 0})</span></h3>
        <div style="display:flex;flex-direction:column;gap:6px">${urgentList}${more((st.urgent || []).length, st.urgent_total || 0)}</div>
      </div>
      <div>
        <h3 style="margin:0 0 8px;font-size:14px">⏰ Échéances proches <span class="muted" style="font-weight:400;font-size:12px">(${st.due_soon_total || 0})</span></h3>
        <div style="display:flex;flex-direction:column;gap:6px">${dueSoonList}${more((st.due_soon || []).length, st.due_soon_total || 0)}</div>
      </div>
    </div>
    <div style="display:grid;grid-template-columns:1fr 1fr;gap:18px">
      <div>
        <h3 style="margin:0 0 8px;font-size:14px">Tableaux</h3>
        <div style="display:flex;flex-direction:column;gap:6px">${rows || '<span class="muted">Aucun tableau</span>'}</div>
        <button class="btn primary sm" id="home-new" style="margin-top:10px">＋ Nouveau board</button>
      </div>
      <div>
        <h3 style="margin:0 0 8px;font-size:14px">Activité récente</h3>
        <div>${acts || '<span class="muted">Aucune activité</span>'}</div>
      </div>
    </div>`;
  box.querySelectorAll('[data-open-board]').forEach((btn) => {
    btn.onclick = () => navigate(boardUrl(btn.dataset.openBoard, 'board'));
  });
  box.querySelectorAll('[data-open-card]').forEach((btn) => {
    btn.onclick = () => {
      const [bid, cid] = btn.dataset.openCard.split('|');
      navigate(`${boardUrl(bid, 'board')}/c/${cid}`);
    };
  });
  box.querySelector('#home-new')?.addEventListener('click', async () => {
    const { promptDialog } = await import('./ui/modal.js');
    const name = await promptDialog('Nom du board', '', 'Ex. : Lancement produit');
    if (!name) return;
    const b = await api.createBoard({ title: name });
    await loadBoards();
    navigate(boardUrl(b.id, 'board'));
  });
}

async function renderRoute() {
  try {
    const route = location.hash;
    if (!app.querySelector('.shell')) {
      app.innerHTML = shellHtml();
      initSidebar(document.getElementById('sidebar'));
      initTopbarShortcuts();
    }
    const full = reloadFull;
    reloadFull = true;
    if (full) await loadBoards();

    if (route.startsWith('#/b/')) {
      const [, , boardId, view = 'board'] = route.split('/');
      try {
        // Toujours recharger : les événements WS et les mutations locales
        // doivent refléter l'état serveur (listes, cartes, deps, schedule).
        await loadBoard(boardId);
        if (full) {
          const allSettings = {};
          try {
            const views = await api.get(`/api/v1/boards/${boardId}/views`);
            for (const v of views) {
              // GET /views renvoie `settings` déjà parsé en objet (le TEXT est
              // déserialisé côté serveur) — gérer aussi une string historique.
              allSettings[v.view] = typeof v.settings === 'string'
                ? JSON.parse(v.settings || '{}')
                : (v.settings || {});
            }
          } catch { /* optionnel */ }
          loadViewSettings(allSettings);
        }
      } catch (e) {
        document.getElementById('main').innerHTML =
          `<div class="empty-state"><div class="big">🌫</div><p>Board introuvable : ${e.message}</p></div>`;
        return;
      }
      state.view = view;
      await hydrateLabelIds();
      renderTopbar(document.getElementById('topbar'));
      const sb = document.getElementById('sidebar');
      if (sb) renderSidebar(sb);
      const main = document.getElementById('main');
      const isSameView = currentBoardId === boardId && currentView === view && main.querySelector('.view');
      currentBoardId = boardId;
      currentView = view;

      if (!isSameView) {
        main.innerHTML = '<div class="view view-enter"></div>';
        switch (view) {
          case 'calendar': renderCalendar(); break;
          case 'timeline': renderTimeline(); break;
          case 'table': renderTable(); break;
          case 'dashboard': renderDashboard(); break;
          case 'settings': {
            renderBoard();
            import('./components/board_settings_modal.js').then((m) => m.openBoardSettingsModal(boardId));
            history.replaceState(null, '', location.pathname + location.search + boardUrl(boardId, 'board'));
            break;
          }
          default: renderBoard();
        }
      } else {
        // Rafraîchissement doux sans détruire le conteneur .view (évite le flash de fenêtre)
        switch (view) {
          case 'calendar': renderCalendar(); break;
          case 'timeline': renderTimeline(); break;
          case 'table': renderTable(); break;
          case 'dashboard': renderDashboard(); break;
          case 'settings': {
            import('./components/board_settings_modal.js').then((m) => m.openBoardSettingsModal(boardId));
            history.replaceState(null, '', location.pathname + location.search + boardUrl(boardId, 'board'));
            break;
          }
          default: {
            if (document.querySelector('#board') && window.__krelloReloadBoard) {
              window.__krelloReloadBoard({ refetch: false });
            } else {
              renderBoard();
            }
            break;
          }
        }
      }

      // Lien profond vers une carte: #/b/{board}/{view}/c/{cardId}.
      // On ouvre la modale puis on retire le segment carte du hash
      // silencieusement (replaceState, sans déclencher le routeur).
      const seg = route.split('/');
      const deepCardId = seg[4] === 'c' ? seg[5] : null;
      if (deepCardId) {
        const { openCardModal } = await import('./components/card_modal.js');
        openCardModal(deepCardId);
        history.replaceState(null, '', location.pathname + location.search + `#/b/${boardId}/${view || 'board'}`);
      }
    } else if (route === '#/settings' || route.startsWith('#/settings')) {
      currentBoardId = null;
      currentView = 'settings';
      state.view = 'settings';
      applyBoardBackground(null);
      renderTopbar(document.getElementById('topbar'));
      const sb = document.getElementById('sidebar');
      if (sb) renderSidebar(sb);
      document.getElementById('main').innerHTML = '<div class="view view-enter"></div>';
      renderSettings();
    } else {
      currentBoardId = null;
      currentView = 'home';
      state.view = 'home';
      // accueil
      state.board = null;
      applyBoardBackground(null);
      renderTopbar(document.getElementById('topbar'));
      const sbHome = document.getElementById('sidebar');
      if (sbHome) renderSidebar(sbHome);
      const main = document.getElementById('main');
      if (!state.boards.length) {
        main.innerHTML = `
          <div class="empty-state">
            <div class="boot-logo" style="width:64px;height:64px;margin:0 auto 20px">${icon('logo', { size: 64 })}</div>
            <h2 style="margin:0 0 8px;font-size:20px;font-weight:700">Bienvenue dans Krello</h2>
            <p style="max-width:460px;margin:0 auto 20px;line-height:1.6">
              Gestionnaire de projets Kanban moderne avec Gantt/CPM, calendrier, automatisations et Apple Intelligence on-device.<br/>
              Appuyez sur <span class="kbd">⌘K</span> pour la palette de commandes.
            </p>
            <button class="btn primary" id="home-new" style="padding:9px 18px;font-size:14px">＋ Créer un board</button>
          </div>`;
        main.querySelector('#home-new').onclick = async () => {
          const { promptDialog } = await import('./ui/modal.js');
          const name = await promptDialog('Nom du board', 'Mon premier board');
          if (!name) return;
          const b = await api.createBoard({ title: name });
          await loadBoards();
          navigate(boardUrl(b.id, 'board'));
        };
        return;
      }
      renderHomeOverview(main);
    }
  } catch (err) {
    console.error('Erreur au chargement de Krello:', err);
    app.innerHTML = `
      <div class="boot" style="max-width: 440px; margin: 80px auto; padding: 32px; background: var(--bg-surface); border: 1px solid var(--border-subtle); border-radius: var(--radius-xl); box-shadow: var(--shadow-lg); text-align: center;">
        <div class="boot-logo" style="width: 56px; height: 56px; margin: 0 auto 16px;">${icon('logo', { size: 56 })}</div>
        <h2 style="font-size: 18px; margin: 0 0 8px; font-weight: 700; color: var(--text-primary);">Connexion à Krello</h2>
        <p style="font-size: 13px; color: var(--text-secondary); margin: 0 0 20px; line-height: 1.5;">
          ${err.message || 'Impossible de joindre le serveur Krello.'}
        </p>
        <div style="display: flex; gap: 10px; justify-content: center;">
          <button class="btn primary" id="btn-retry" style="padding: 8px 16px; font-size: 13px;">Réessayer</button>
          <button class="btn" id="btn-reload" style="padding: 8px 16px; font-size: 13px;">Recharger</button>
        </div>
      </div>`;
    document.getElementById('btn-retry')?.addEventListener('click', () => {
      app.innerHTML = `<div class="boot"><div class="boot-logo">${icon('logo', { size: 56 })}</div><p>Chargement de Krello…</p></div>`;
      renderRoute();
    });
    document.getElementById('btn-reload')?.addEventListener('click', () => location.reload());
  }
}

// Réactivité WS : rechargement ciblé selon les événements serveur.
let reloadTimer = null;
let reloadFull = true;
function scheduleReload(full = true) {
  reloadFull = full;
  clearTimeout(reloadTimer);
  reloadTimer = setTimeout(() => renderRoute(), 150);
}

on('*', (evt) => {
  const t = evt.type;
  // Seuls les événements de board / resync justifient de recharger la liste des
  // boards et les vues enregistrées ; un event carte/liste ne refait que
  // loadBoard + le re-render de la vue courante.
  const full = ['BoardCreated', 'BoardDeleted', 'BoardUpdated', 'resync',
    'AutomationsUpdated', 'AutomationRan', 'BaselinesUpdated', 'CalendarsUpdated'].includes(t);
  if (full || ['CardCreated', 'CardUpdated', 'CardMoved', 'CardDeleted', 'LabelsUpdated',
    'ChecklistsUpdated', 'CommentsUpdated', 'AttachmentsUpdated', 'FieldsUpdated',
    'DependenciesUpdated', 'ScheduleUpdated', 'UndoApplied',
    'ListCreated', 'ListUpdated', 'ListDeleted'].includes(t)) {
    scheduleReload(full);
  }
});

// Rappels locaux : le sweep minute émet ReminderDue une seule fois par
// carte/échéance (title/due voyagent avec l'événement, pas de refetch).
on('ReminderDue', ({ title, due }) => {
  const d = (due || '').slice(0, 16).replace('T', ' ');
  toast(`🔔 Rappel — « ${title} » (échéance ${d})`, 'ai');
});

// Accessibilité : réplique title → aria-label sur les contrôles sans libellé
// accessible (l'app génère beaucoup de boutons icônes via innerHTML).
new MutationObserver((muts) => {
  // Ne traiter que les nœuds réellement ajoutés (et leurs descendants) :
  // scanner tout document.body à chaque mutation costait cher pendant les drags.
  const labelIfIcon = (n) => {
    if (n.nodeType !== 1) return;
    if (n.matches('[title]:not([aria-label])') && n.matches('.iconbtn, button, [role="button"]') && !n.textContent.trim()) {
      n.setAttribute('aria-label', n.getAttribute('title'));
    }
    n.querySelectorAll?.('[title]:not([aria-label])').forEach((c) => {
      if (c.matches('.iconbtn, button, [role="button"]') && !c.textContent.trim()) {
        c.setAttribute('aria-label', c.getAttribute('title'));
      }
    });
  };
  for (const m of muts) {
    for (const n of m.addedNodes) labelIfIcon(n);
  }
}).observe(document.body, { childList: true, subtree: true });

emit('app.boot');
startWs();
initDeepLink();
initPalette();
startRouter(renderRoute);
