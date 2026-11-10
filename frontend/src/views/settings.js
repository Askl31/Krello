// Vue Paramètres : labels, champs personnalisés, calendriers, automations, données, IA.

import { api } from '../api.js';
import { escapeHtml, LABEL_COLORS } from '../format.js';
import { state, loadBoard, loadBoards } from '../state.js';
import { toast, confirmDialog, promptDialog, openModal } from '../ui/modal.js';
import { boardUrl, navigate } from '../router.js';

export function renderSettings() {
  const host = document.querySelector('.view');
  const board = state.board;

  host.innerHTML = `
    <h2 style="margin-top:0">Paramètres</h2>
    <div class="settings-grid">
      <div class="panel">
        <h3>Assistant IA — Apple Intelligence</h3>
        <div id="st-ai"><span class="muted">…</span></div>
      </div>

      ${board ? `
      <div class="panel">
        <h3>Board « ${escapeHtml(board.title)} »</h3>
        <div class="field"><label>Démarrage projet (CPM)</label><input type="date" class="input" id="st-start" value="${board.start_date || ''}" /></div>
        <div class="field"><label>Échéance projet</label><input type="date" class="input" id="st-deadline" value="${board.deadline || ''}" /></div>
        <div class="field"><label>Calendrier</label>
          <select class="input" id="st-calendar"><option value="default">Standard (week-end sam/dim)</option></select>
        </div>
        <div class="row">
          <button class="btn primary sm" id="st-board-save">Enregistrer</button>
          <button class="btn sm" id="st-board-duplicate">Dupliquer</button>
          <button class="btn sm" id="st-board-export-json">Export JSON</button>
          <button class="btn sm" id="st-board-export-md">Export MD</button>
          <button class="btn sm" id="st-board-export-csv">Export CSV</button>
          <button class="btn sm" id="st-board-export-ics">Export ICS</button>
          <button class="btn danger sm" id="st-board-delete">Supprimer</button>
        </div>
      </div>

      <div class="panel">
        <h3>Labels</h3>
        <div id="st-labels"></div>
        <button class="btn sm" id="st-add-label" style="margin-top:8px">＋ Label</button>
      </div>

      <div class="panel">
        <h3>Champs personnalisés</h3>
        <div id="st-fields"></div>
        <button class="btn sm" id="st-add-field" style="margin-top:8px">＋ Champ</button>
      </div>

      <div class="panel">
        <h3>Automatisations</h3>
        <div id="st-automations"></div>
        <button class="btn sm" id="st-add-automation" style="margin-top:8px">＋ Règle</button>
      </div>` : '<div class="panel"><h3>Board</h3><span class="muted">Ouvrez un board pour éditer ses paramètres.</span></div>'}

      <div class="panel">
        <h3>Calendriers de travail</h3>
        <div id="st-calendars"></div>
        <button class="btn sm" id="st-add-calendar" style="margin-top:8px">＋ Calendrier</button>
      </div>
    </div>`;

  // --- IA
  api.aiStatus().then((s) => {
    host.querySelector('#st-ai').innerHTML = s.available
      ? `<span class="badge done">● disponible</span> <span class="muted">backend : ${s.backend} — tout est local, aucune donnée ne quitte la machine.</span>`
      : `<span class="badge overdue">● indisponible</span> <span class="muted">${s.reason || ''} — requiert Apple Silicon + Apple Intelligence activée (Réglages › Apple Intelligence &amp; Siri) et macOS 15.1+.</span>`;
  });

  if (!board) return;
  const boardId = board.id;

  // --- board
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
    if (await confirmDialog(`Supprimer « ${board.title} » ?`)) {
      await api.deleteBoard(boardId);
      await loadBoards();
      navigate('#/');
    }
  };

  // --- labels
  const labelsHost = host.querySelector('#st-labels');
  labelsHost.innerHTML = state.labels.map((l) => `
    <div class="row" style="margin-bottom:6px" data-label="${l.id}">
      <span class="chip-label" style="background:${l.color};width:60px;text-align:center">${escapeHtml(l.name || '•')}</span>
      <input class="input" data-rename value="${escapeHtml(l.name)}" placeholder="Nom du label" />
      <button class="iconbtn" data-del>✕</button>
    </div>`).join('');
  labelsHost.addEventListener('change', async (e) => {
    const row = e.target.closest('[data-label]');
    if (row && e.target.matches('[data-rename]')) {
      await api.updateLabel(row.dataset.label, { name: e.target.value });
      await loadBoard(boardId);
      toast('Label renommé', 'ok');
    }
  });
  labelsHost.addEventListener('click', async (e) => {
    if (e.target.matches('[data-del], [data-del] *')) {
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
    <div class="row" style="margin-bottom:6px">
      <b>${escapeHtml(f.name)}</b><span class="muted">${f.kind}</span>
      <button class="iconbtn" data-del-field="${f.id}" style="width:22px;height:22px;font-size:11px">✕</button>
    </div>`).join('') || '<span class="muted">Aucun champ</span>';
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
      <div class="row" style="margin-bottom:6px">
        <span style="flex:1"><b>${escapeHtml(a.name)}</b> <span class="badge">${a.trigger}</span>
          <span class="muted" style="font-size:11px;display:block">${escapeHtml(a.actions)}</span></span>
        <button class="btn sm" data-run="${a.id}">▶</button>
        <button class="iconbtn" data-del-auto="${a.id}" style="width:22px;height:22px;font-size:11px">🗑</button>
      </div>`).join('') || '<span class="muted">Aucune règle</span>';
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
      <div class="modal-head"><h2>Nouvelle automatisation</h2></div>
      <div class="modal-body">
        <div class="field"><label>Nom</label><input class="input" id="au-name" value="Ma règle" /></div>
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
        <div class="field"><label>Conditions (JSON)</label><textarea class="input" id="au-cond" style="font-family:var(--mono);font-size:12px">{"list_id": null}</textarea></div>
        <div class="field"><label>Actions (JSON)</label><textarea class="input" id="au-actions" style="font-family:var(--mono);font-size:12px">[{"type":"priority","value":"high"}]</textarea></div>
        <div class="row" style="justify-content:flex-end"><button class="btn primary" id="au-create">Créer</button></div>
      </div>`);
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
      <div class="row" style="margin-bottom:6px">
        <b style="flex:1">${escapeHtml(c.name)}</b>
        <span class="muted">${c.hours_per_day} h/j</span>
        <button class="iconbtn" data-del-cal="${c.id}" ${c.id === 'default' ? 'disabled style="opacity:.3"' : ''} style="width:22px;height:22px;font-size:11px">🗑</button>
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
