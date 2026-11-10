// Vue Paramètres : labels, champs personnalisés, calendriers, automations, données, IA.

import { api } from '../api.js';
import { escapeHtml, LABEL_COLORS } from '../format.js';
import { state, applyTheme, loadBoard, loadBoards, applyBoardBackground } from '../state.js';
import { toast, confirmDialog, promptDialog, openModal } from '../ui/modal.js';
import { boardUrl, navigate } from '../router.js';
import { icon } from '../icons.js';

export const BOARD_BACKGROUNDS = [
  // Arrière-plans sombres
  { name: 'Obsidian Noir', value: 'linear-gradient(135deg, #090d16 0%, #111827 100%)' },
  { name: 'Deep Aurora', value: 'linear-gradient(135deg, #09131f 0%, #062b33 50%, #0d1e38 100%)' },
  { name: 'Midnight Indigo', value: 'linear-gradient(135deg, #0c0f2b 0%, #18194a 50%, #1e1b4b 100%)' },
  { name: 'Sunset Velvet', value: 'linear-gradient(135deg, #1c0d1e 0%, #2b112c 50%, #1a0b18 100%)' },
  { name: 'Emerald Forest', value: 'linear-gradient(135deg, #061914 0%, #0a261f 50%, #041410 100%)' },
  { name: 'Cyber Neon', value: 'linear-gradient(135deg, #140b24 0%, #07192c 100%)' },

  // Arrière-plans clairs
  { name: 'Studio Brume', value: 'linear-gradient(135deg, #f8fafc 0%, #e2e8f0 100%)' },
  { name: 'Aurore Givrée', value: 'linear-gradient(135deg, #f0fdf4 0%, #e0f2fe 50%, #f1f5f9 100%)' },
  { name: 'Lavande Douce', value: 'linear-gradient(135deg, #f5f3ff 0%, #ede9fe 50%, #faf5ff 100%)' },
  { name: 'Pêche Poudrée', value: 'linear-gradient(135deg, #fff7ed 0%, #ffedd5 50%, #fef2f2 100%)' },
  { name: 'Bleu Azur', value: 'linear-gradient(135deg, #f0f9ff 0%, #e0f2fe 50%, #dbeafe 100%)' },
  { name: 'Menthe Fraîche', value: 'linear-gradient(135deg, #ecfdf5 0%, #d1fae5 50%, #f0fdfa 100%)' },
];

export function renderSettings() {
  const host = document.querySelector('.view');
  const board = state.board;

  host.innerHTML = `
    <h2 style="margin-top:0">Paramètres de l'application</h2>
    <div class="settings-grid">
      <div class="panel">
        <h3 style="display:flex;align-items:center;gap:8px">
          ${icon('theme', { size: 16, className: 'muted' })}
          <span>Apparence & Thème</span>
        </h3>
        <p class="muted" style="margin:0 0 12px;font-size:12.5px">Choisissez le mode d'affichage de Krello selon votre environnement.</p>
        <div class="viewtabs" id="st-theme-picker" style="width:100%;display:flex">
          <button data-theme="dark" class="${state.theme === 'dark' ? 'active' : ''}" style="flex:1;justify-content:center">
            ${icon('moon', { size: 14 })} <span>Sombre</span>
          </button>
          <button data-theme="light" class="${state.theme === 'light' ? 'active' : ''}" style="flex:1;justify-content:center">
            ${icon('sun', { size: 14 })} <span>Clair</span>
          </button>
          <button data-theme="auto" class="${state.theme === 'auto' ? 'active' : ''}" style="flex:1;justify-content:center">
            ${icon('monitor', { size: 14 })} <span>Système</span>
          </button>
        </div>
      </div>

      <div class="panel">
        <h3 style="display:flex;align-items:center;gap:8px">
          ${icon('ai', { size: 16, className: 'muted' })}
          <span>Assistant IA — Apple Intelligence</span>
        </h3>
        <div id="st-ai"><span class="muted">Vérification de la disponibilité…</span></div>
      </div>

      ${board ? `
      <div class="panel">
        <h3 style="display:flex;align-items:center;gap:8px">
          ${icon('boards', { size: 16, className: 'muted' })}
          <span>Configuration du Board « ${escapeHtml(board.title)} »</span>
        </h3>
        <div class="field"><label>Démarrage projet (calcul CPM)</label><input type="date" class="input" id="st-start" value="${board.start_date || ''}" /></div>
        <div class="field"><label>Échéance globale projet</label><input type="date" class="input" id="st-deadline" value="${board.deadline || ''}" /></div>
        <div class="field"><label>Calendrier de travail</label>
          <select class="input" id="st-calendar"><option value="default">Standard (week-end sam/dim)</option></select>
        </div>
        <div class="field" style="margin-top:14px">
          <label>Ambiance & Thème du Board</label>
          <div class="swatches" id="st-bg-swatches" style="margin-top:6px;gap:8px">
            <button data-bg="${escapeHtml(JSON.stringify({ type: 'default', value: '#101828' }))}"
                    title="Par défaut (s'adapte au mode clair ou sombre)"
                    class="${!board.background || board.background.includes('#101828') ? 'sel' : ''}"
                    style="background:var(--surface-2);border:1px dashed var(--border-strong);display:grid;place-items:center;font-size:11px;font-weight:600;color:var(--muted);width:34px;height:34px;border-radius:var(--radius-xs)">auto</button>
            ${BOARD_BACKGROUNDS.map((bg) => `
              <button data-bg="${escapeHtml(JSON.stringify({ type: 'gradient', value: bg.value }))}"
                      title="${bg.name}"
                      class="${board.background?.includes(bg.value) ? 'sel' : ''}"
                      style="background:${bg.value};width:34px;height:34px;border-radius:var(--radius-xs)"></button>
            `).join('')}
          </div>
        </div>
        <div class="row" style="flex-wrap:wrap;gap:6px;margin-top:16px">
          <button class="btn primary sm" id="st-board-save">Enregistrer</button>
          <button class="btn sm" id="st-board-duplicate">${icon('copy', { size: 13 })} <span>Dupliquer</span></button>
          <button class="btn sm" id="st-board-export-json">Export JSON</button>
          <button class="btn sm" id="st-board-export-md">Export MD</button>
          <button class="btn sm" id="st-board-export-csv">Export CSV</button>
          <button class="btn sm" id="st-board-export-ics">Export ICS</button>
          <button class="btn danger sm" id="st-board-delete">${icon('trash', { size: 13 })} <span>Supprimer</span></button>
        </div>
      </div>

      <div class="panel">
        <h3>Labels du board</h3>
        <div id="st-labels"></div>
        <button class="btn sm" id="st-add-label" style="margin-top:10px">
          ${icon('plus', { size: 13 })} <span>Nouveau label</span>
        </button>
      </div>

      <div class="panel">
        <h3>Champs personnalisés</h3>
        <div id="st-fields"></div>
        <button class="btn sm" id="st-add-field" style="margin-top:10px">
          ${icon('plus', { size: 13 })} <span>Nouveau champ</span>
        </button>
      </div>

      <div class="panel">
        <h3>Automatisations locales</h3>
        <div id="st-automations"></div>
        <button class="btn sm" id="st-add-automation" style="margin-top:10px">
          ${icon('plus', { size: 13 })} <span>Nouvelle règle</span>
        </button>
      </div>` : '<div class="panel"><h3>Board</h3><span class="muted">Ouvrez un board pour éditer ses paramètres spécifiques.</span></div>'}

      <div class="panel">
        <h3>Calendriers de travail</h3>
        <div id="st-calendars"></div>
        <button class="btn sm" id="st-add-calendar" style="margin-top:10px">
          ${icon('plus', { size: 13 })} <span>Nouveau calendrier</span>
        </button>
      </div>
    </div>`;

  // --- Thème
  host.querySelector('#st-theme-picker')?.addEventListener('click', (e) => {
    const btn = e.target.closest('[data-theme]');
    if (!btn) return;
    state.theme = btn.dataset.theme;
    applyTheme();
    const tb = document.getElementById('topbar');
    if (tb) {
      import('../components/topbar.js').then((m) => m.renderTopbar(tb));
    }
    renderSettings();
  });

  // --- IA
  api.aiStatus().then((s) => {
    host.querySelector('#st-ai').innerHTML = s.available
      ? `<span class="badge done">● Disponible</span> <p class="muted" style="margin:8px 0 0;font-size:12.5px">Moteur Apple Intelligence actif (${s.backend}). 100 % on-device — aucune donnée ne quitte votre appareil.</p>`
      : `<span class="badge overdue">● Indisponible</span> <p class="muted" style="margin:8px 0 0;font-size:12.5px">${s.reason || ''} — Nécessite macOS 15.1+ sur Mac Apple Silicon avec Apple Intelligence activée dans Réglages Système.</p>`;
  });

  if (!board) return;
  const boardId = board.id;

  // --- board
  host.querySelector('#st-bg-swatches')?.addEventListener('click', async (e) => {
    const btn = e.target.closest('[data-bg]');
    if (!btn) return;
    const bg = btn.dataset.bg;
    try {
      await api.updateBoard(boardId, { background: bg });
      await loadBoard(boardId);
      renderSettings();
      toast('Ambiance du board modifiée', 'ok');
    } catch (err) { toast(err.message, 'err'); }
  });

  host.querySelector('#st-board-save').onclick = async () => {
    try {
      await api.updateBoard(boardId, {
        start_date: host.querySelector('#st-start').value || null,
        deadline: host.querySelector('#st-deadline').value || null,
      });
      toast('Board enregistré — ordonnancement recalculé', 'ok');
      await loadBoard(boardId);
    } catch (e) { toast(e.message, 'err'); }
  };
  host.querySelector('#st-board-duplicate').onclick = async () => {
    const b = await api.duplicateBoard(boardId);
    await loadBoards();
    navigate(boardUrl(b.id, 'board'));
    toast('Board dupliqué', 'ok');
  };
  for (const fmt of ['json', 'md', 'csv', 'ics']) {
    host.querySelector(`#st-board-export-${fmt}`).onclick = () => {
      window.open(`/api/v1/boards/${boardId}/export?format=${fmt}`, '_blank');
    };
  }
  host.querySelector('#st-board-delete').onclick = async () => {
    if (await confirmDialog(`Supprimer définitivement le board « ${board.title} » ?`)) {
      await api.deleteBoard(boardId);
      await loadBoards();
      navigate('#/');
    }
  };

  // --- labels
  const labelsHost = host.querySelector('#st-labels');
  labelsHost.innerHTML = state.labels.map((l) => `
    <div class="row" style="margin-bottom:6px;gap:8px" data-label="${l.id}">
      <span class="chip-label" style="background:${l.color};min-width:60px;text-align:center">${escapeHtml(l.name || '•')}</span>
      <input class="input" data-rename value="${escapeHtml(l.name)}" placeholder="Nom du label" style="padding:4px 8px;font-size:12.5px" />
      <button class="iconbtn" data-del title="Supprimer" style="width:24px;height:24px">${icon('close', { size: 12 })}</button>
    </div>`).join('');
  labelsHost.addEventListener('change', async (e) => {
    const row = e.target.closest('[data-label]');
    if (row && e.target.matches('[data-rename]')) {
      await api.updateLabel(row.dataset.label, { name: e.target.value });
      await loadBoard(boardId);
      toast('Label mis à jour', 'ok');
    }
  });
  labelsHost.addEventListener('click', async (e) => {
    if (e.target.closest('[data-del]')) {
      const row = e.target.closest('[data-label]');
      await api.deleteLabel(row.dataset.label);
      await loadBoard(boardId);
      renderSettings();
    }
  });
  host.querySelector('#st-add-label').onclick = async () => {
    const name = await promptDialog('Nom du label');
    if (!name) return;
    const color = LABEL_COLORS[state.labels.length % LABEL_COLORS.length];
    await api.createLabel(boardId, { name, color });
    await loadBoard(boardId);
    renderSettings();
  };

  // --- champs
  const fieldsHost = host.querySelector('#st-fields');
  fieldsHost.innerHTML = state.fields.map((f) => `
    <div class="row" style="margin-bottom:6px;padding:4px 8px;border-radius:var(--radius-xs);background:var(--surface-2)">
      <b style="font-size:13px">${escapeHtml(f.name)}</b>
      <span class="badge" style="margin-left:auto">${f.kind}</span>
      <button class="iconbtn" data-del-field="${f.id}" title="Supprimer" style="width:22px;height:22px">${icon('close', { size: 12 })}</button>
    </div>`).join('') || '<span class="muted" style="font-size:12.5px">Aucun champ personnalisé</span>';
  fieldsHost.querySelectorAll('[data-del-field]').forEach((b) => {
    b.onclick = async () => { await api.deleteField(b.dataset.delField); await loadBoard(boardId); renderSettings(); };
  });
  host.querySelector('#st-add-field').onclick = async () => {
    const name = await promptDialog('Nom du champ');
    if (!name) return;
    const kind = await promptDialog('Type (text, number, date, checkbox, select)', 'text');
    if (!kind) return;
    try {
      await api.createField(boardId, { name, kind });
      await loadBoard(boardId);
      renderSettings();
    } catch (e) { toast(e.message, 'err'); }
  };

  // --- automations
  const autosHost = host.querySelector('#st-automations');
  api.automations(boardId).then((autos) => {
    autosHost.innerHTML = autos.map((a) => `
      <div class="row" style="margin-bottom:8px;padding:6px 10px;border-radius:var(--radius-xs);background:var(--surface-2)">
        <span style="flex:1">
          <b>${escapeHtml(a.name)}</b> <span class="badge">${a.trigger}</span>
          <span class="muted" style="font-size:11px;display:block;margin-top:2px">${escapeHtml(a.actions)}</span>
        </span>
        <button class="btn sm" data-run="${a.id}" title="Exécuter maintenant">▶ Lancer</button>
        <button class="iconbtn" data-del-auto="${a.id}" title="Supprimer" style="width:22px;height:22px">${icon('trash', { size: 12 })}</button>
      </div>`).join('') || '<span class="muted" style="font-size:12.5px">Aucune règle définie</span>';
    autosHost.querySelectorAll('[data-run]').forEach((b) => {
      b.onclick = async () => {
        const r = await api.runAutomation(b.dataset.run);
        toast(`${r.cards_touched} carte(s) traitée(s)`, 'ok');
        await loadBoard(boardId);
      };
    });
    autosHost.querySelectorAll('[data-del-auto]').forEach((b) => {
      b.onclick = async () => { await api.deleteAutomation(b.dataset.delAuto); renderSettings(); };
    });
  });
  host.querySelector('#st-add-automation').onclick = () => {
    const { el, close } = openModal(`
      <div class="dialog-header">
        <div class="dialog-badge">${icon('settings', { size: 18 })}</div>
        <div class="dialog-titles">
          <h3 class="dialog-title">Nouvelle règle d'automatisation</h3>
          <p class="dialog-desc">Automatisez les actions répétitives sur vos cartes en local.</p>
        </div>
        <button class="iconbtn dialog-close" data-close title="Fermer (Échap)">${icon('close', { size: 14 })}</button>
      </div>
      <div class="dialog-body">
        <div class="field"><label>Nom de la règle</label><input class="input" id="au-name" value="Ma règle" placeholder="Ex. : Déplacer les cartes urgentes" /></div>
        <div class="field"><label>Déclencheur</label>
          <select class="input" id="au-trigger">
            <option value="card_created">card_created — carte créée</option>
            <option value="card_moved">card_moved — carte déplacée</option>
            <option value="card_labeled">card_labeled — label ajouté</option>
            <option value="card_due_soon">card_due_soon — échéance proche</option>
            <option value="button">button — bouton manuel</option>
            <option value="scheduled_daily">scheduled_daily — quotidien</option>
            <option value="scheduled_weekly">scheduled_weekly — hebdomadaire</option>
          </select>
        </div>
        <div class="field"><label>Conditions (JSON)</label><textarea class="input" id="au-cond" style="font-family:var(--mono);font-size:12px;min-height:50px">{"list_id": null}</textarea></div>
        <div class="field"><label>Actions (JSON)</label><textarea class="input" id="au-actions" style="font-family:var(--mono);font-size:12px;min-height:50px">[{"type":"priority","value":"high"}]</textarea></div>
      </div>
      <div class="dialog-footer">
        <div class="dialog-hints">
          <span class="kbd">Échap</span> Annuler
        </div>
        <div class="dialog-btns">
          <button class="btn" data-close>Annuler</button>
          <button class="btn primary" id="au-create">Créer la règle</button>
        </div>
      </div>`, { size: 'md' });
    el.querySelector('#au-create').onclick = async () => {
      try {
        await api.createAutomation(boardId, {
          name: el.querySelector('#au-name').value,
          trigger: el.querySelector('#au-trigger').value,
          conditions: el.querySelector('#au-cond').value,
          actions: el.querySelector('#au-actions').value,
        });
        close();
        renderSettings();
      } catch (e) { toast(e.message, 'err'); }
    };
  };

  // --- calendriers
  const calHost = host.querySelector('#st-calendars');
  api.calendars().then((cals) => {
    const select = host.querySelector('#st-calendar');
    select.innerHTML = cals.map((c) => `<option value="${c.id}" ${c.id === board.calendar_id ? 'selected' : ''}>${escapeHtml(c.name)}</option>`).join('');
    calHost.innerHTML = cals.map((c) => `
      <div class="row" style="margin-bottom:6px;padding:4px 8px;border-radius:var(--radius-xs);background:var(--surface-2)">
        <b style="flex:1">${escapeHtml(c.name)}</b>
        <span class="muted">${c.hours_per_day} h/j</span>
        <button class="iconbtn" data-del-cal="${c.id}" ${c.id === 'default' ? 'disabled style="opacity:.3"' : ''} title="Supprimer" style="width:22px;height:22px">${icon('trash', { size: 12 })}</button>
      </div>`).join('');
    calHost.querySelectorAll('[data-del-cal]').forEach((b) => {
      b.onclick = async () => {
        try { await api.deleteCalendar(b.dataset.delCal); renderSettings(); }
        catch (e) { toast(e.message, 'err'); }
      };
    });
    select.onchange = async () => {
      await api.updateBoard(boardId, { calendar_id: select.value });
      await loadBoard(boardId);
      toast('Calendrier appliqué — recalcul CPM', 'ok');
    };
  });
  host.querySelector('#st-add-calendar').onclick = async () => {
    const name = await promptDialog('Nom du calendrier');
    if (!name) return;
    const holidays = await promptDialog('Jours fériés (JSON, ex. ["2027-01-01"])', '[]');
    try {
      await api.createCalendar({ name, holidays });
      renderSettings();
    } catch (e) { toast(e.message, 'err'); }
  };
}
