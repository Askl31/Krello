// Bootstrap Krello : shell, router, WS, raccourcis clavier.

import { api } from './api.js';
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
          case 'settings': renderSettings(); break;
          default: renderBoard();
        }
      } else {
        // Rafraîchissement doux sans détruire le conteneur .view (évite le flash de fenêtre)
        switch (view) {
          case 'calendar': renderCalendar(); break;
          case 'timeline': renderTimeline(); break;
          case 'table': renderTable(); break;
          case 'dashboard': renderDashboard(); break;
          case 'settings': renderSettings(); break;
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
        const { openCardModal } = await import('../components/card_modal.js');
        openCardModal(deepCardId);
        history.replaceState(null, '', location.pathname + location.search + `#/b/${boardId}/${view || 'board'}`);
      }
    } else if (route === '#/settings') {
      if (state.board) {
        navigate(boardUrl(state.board.id, 'settings'));
        return;
      }
      if (state.boards.length > 0) {
        navigate(boardUrl(state.boards[0].id, 'settings'));
        return;
      }
      currentBoardId = null;
      currentView = 'settings';
      state.view = 'settings';
      state.board = null;
      applyBoardBackground(null);
      renderTopbar(document.getElementById('topbar'));
      const sb = document.getElementById('sidebar');
      if (sb) renderSidebar(sb);
      document.getElementById('main').innerHTML = '<div class="view view-enter"></div>';
      renderSettings();
    } else {
      currentBoardId = null;
      currentView = 'home';
      // accueil
      state.board = null;
      applyBoardBackground(null);
      renderTopbar(document.getElementById('topbar'));
      const main = document.getElementById('main');
      if (state.boards.length) {
        navigate(boardUrl(state.boards[0].id, 'board'));
        return;
      }
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
