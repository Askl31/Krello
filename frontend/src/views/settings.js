// Vue Paramètres : labels, champs personnalisés, calendriers, automations, données, IA.

import { api } from '../api.js';
import { escapeHtml, LABEL_COLORS } from '../format.js';
import { state, applyTheme, applyStyle, loadBoard, loadBoards, applyBoardBackground } from '../state.js';
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

// ---------- Automatisations (moteur « bot » local, façon Butler) ----------

const TRIGGERS_UI = [
  ['card_created', 'Carte créée'],
  ['card_moved', 'Carte déplacée'],
  ['card_labeled', 'Label ajouté'],
  ['card_unlabeled', 'Label retiré'],
  ['card_archived', 'Carte archivée'],
  ['card_commented', 'Commentaire ajouté'],
  ['checklist_item_checked', 'Case de checklist cochée'],
  ['card_due_soon', 'Échéance proche (balayage)'],
  ['card_due', 'Échéance atteinte (balayage)'],
  ['button', 'Bouton manuel'],
  ['scheduled_daily', 'Tous les jours (balayage)'],
  ['scheduled_weekly', 'Toutes les semaines (balayage)'],
  ['scheduled_monthly', 'Tous les mois (balayage)'],
];

const ACTIONS_UI = [
  ['move', 'Déplacer vers une liste'],
  ['copy', 'Copier vers une liste / un autre board'],
  ['create_card', 'Créer une carte (liste/board au choix)'],
  ['label', 'Ajouter un label'],
  ['unlabel', 'Retirer un label'],
  ['due', 'Définir l’échéance'],
  ['clear_due', 'Supprimer l’échéance'],
  ['priority', 'Changer la priorité'],
  ['check', 'Ajouter un élément de checklist'],
  ['check_item', 'Cocher un élément existant'],
  ['archive', 'Archiver la carte'],
  ['unarchive', 'Désarchiver la carte'],
  ['comment', 'Ajouter un commentaire'],
  ['set_title', 'Renommer la carte'],
  ['set_field', 'Remplir un champ personnalisé'],
];

const PRIORITIES_UI = [['none', 'Aucune'], ['low', 'Basse'], ['medium', 'Moyenne'], ['high', 'Haute'], ['urgent', 'Urgente']];
const WEEKDAYS_UI = [[1, 'Lundi'], [2, 'Mardi'], [3, 'Mercredi'], [4, 'Jeudi'], [5, 'Vendredi'], [6, 'Samedi'], [7, 'Dimanche']];
const MOST_TRIGGERS = 'card_created card_moved card_archived card_commented checklist_item_checked card_due_soon card_due button scheduled_daily scheduled_weekly scheduled_monthly';

function triggerLabel(t) { return (TRIGGERS_UI.find((x) => x[0] === t) || [t, t])[1]; }
function actionLabel(t) { return (ACTIONS_UI.find((x) => x[0] === t) || [t, t])[1]; }
function prioLabel(p) { return (PRIORITIES_UI.find((x) => x[0] === p) || [p, p])[1]; }

function findList(listsCache, listId) {
  for (const ls of Object.values(listsCache)) { const l = (ls || []).find((x) => x.id === listId); if (l) return l; }
  return null;
}
function findLabel(labels, id) { return (labels || []).find((l) => l.id === id); }
function findField(fields, id) { return (fields || []).find((f) => f.id === id); }

function describeConditions(raw, listsCache, labels) {
  let c; try { c = JSON.parse(raw || '{}'); } catch { return raw || ''; }
  const p = [];
  if (c.list_id) p.push(`liste « ${findList(listsCache, c.list_id)?.title || c.list_id} »`);
  if (Array.isArray(c.list_ids) && c.list_ids.length) p.push(`listes : ${c.list_ids.map((id) => findList(listsCache, id)?.title || id).join(', ')}`);
  if (c.from_list_id) p.push(`depuis « ${findList(listsCache, c.from_list_id)?.title || c.from_list_id} »`);
  if (c.to_list_id) p.push(`vers « ${findList(listsCache, c.to_list_id)?.title || c.to_list_id} »`);
  if (c.label_id) p.push(`label « ${findLabel(labels, c.label_id)?.name || c.label_id} »`);
  if (c.without_label_id) p.push(`sans label « ${findLabel(labels, c.without_label_id)?.name || c.without_label_id} »`);
  if (c.title_contains) p.push(`titre contient « ${c.title_contains} »`);
  if (c.description_contains) p.push(`description contient « ${c.description_contains} »`);
  if (c.priority) p.push(`priorité ${prioLabel(c.priority)}`);
  if (c.has_due === true) p.push('avec échéance');
  if (c.has_due === false) p.push('sans échéance');
  if (c.overdue === true) p.push('en retard');
  if (c.days != null) p.push(`échéance dans ≤ ${c.days} j`);
  if (c.time) p.push(`à ${c.time}`);
  if (c.weekday) p.push(WEEKDAYS_UI.find((w) => w[0] === c.weekday)?.[1] || `jour ${c.weekday}`);
  if (c.day) p.push(`jour du mois ${c.day}`);
  return p.join(' · ');
}

function describeAction(a, listsCache, labels, fields) {
  const listTxt = a.list_id ? `« ${findList(listsCache, a.list_id)?.title || a.list_id} »` : '';
  switch (a.type) {
    case 'move': return `déplacer vers ${listTxt}${a.position === 'top' ? ' (en haut)' : ''}`;
    case 'copy': return `copier vers ${listTxt}${a.title ? ` (« ${a.title} »)` : ''}`;
    case 'create_card': return `créer « ${a.title} » dans ${listTxt}`;
    case 'label': return `ajouter le label « ${findLabel(labels, a.label_id)?.name || a.label_id} »`;
    case 'unlabel': return `retirer le label « ${findLabel(labels, a.label_id)?.name || a.label_id} »`;
    case 'due': return a.date ? `échéance le ${a.date}${a.time ? ` à ${a.time}` : ''}` : `échéance dans ${a.offset_days ?? 0} j${a.time ? ` à ${a.time}` : ''}`;
    case 'clear_due': return 'supprimer l’échéance';
    case 'priority': return `priorité → ${prioLabel(a.value)}`;
    case 'check': return `ajouter « ${a.item_title || 'Action requise'} » à la checklist`;
    case 'check_item': return `cocher « ${a.item_title} »`;
    case 'archive': return 'archiver la carte';
    case 'unarchive': return 'désarchiver la carte';
    case 'comment': return `commenter « ${a.text} »`;
    case 'set_title': return `renommer en « ${a.value} »`;
    case 'set_field': return `champ « ${findField(fields, a.field_id)?.name || a.field_id} » = ${a.value}`;
    default: return a.type;
  }
}

// Panneau des règles du board courant.
async function renderAutomations(host, boardId) {
  const lists = state.lists || [];
  const labels = state.labels || [];
  const fields = state.fields || [];
  const boards = state.boards?.length ? state.boards : await api.boards();
  const listsCache = { [boardId]: lists };
  await Promise.all(boards.map(async (b) => {
    if (!listsCache[b.id]) { try { listsCache[b.id] = await api.lists(b.id); } catch { listsCache[b.id] = []; } }
  }));
  const autos = await api.automations(boardId);
  host.innerHTML = autos.map((a) => {
    let acts = []; try { acts = JSON.parse(a.actions || '[]'); } catch { /* ignore */ }
    const cond = describeConditions(a.conditions, listsCache, labels);
    const summary = `${cond || 'toutes les cartes'} → ${acts.map((x) => describeAction(x, listsCache, labels, fields)).join(' ; ') || 'aucune action'}`;
    return `
      <div class="auto-rule" style="margin-bottom:8px;padding:8px 10px;border-radius:var(--radius-xs);background:var(--surface-2)">
        <div class="row">
          <button class="btn sm ${a.enabled ? '' : ''}" data-toggle="${a.id}" title="Activer / désactiver"
                  style="${a.enabled ? '' : 'opacity:.55'}">${a.enabled ? 'Activée' : 'Coupée'}</button>
          <span style="flex:1;min-width:0">
            <b>${escapeHtml(a.name)}</b> <span class="badge">${escapeHtml(triggerLabel(a.trigger))}</span>
            <span class="muted" style="font-size:11px;display:block;margin-top:2px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap" title="${escapeHtml(summary)}">${escapeHtml(summary)}</span>
          </span>
          <button class="btn sm" data-run="${a.id}" title="Exécuter maintenant">▶ Lancer</button>
          <button class="iconbtn" data-edit="${a.id}" title="Modifier" style="width:22px;height:22px">${icon('edit', { size: 12 })}</button>
          <button class="iconbtn" data-runs="${a.id}" title="Journal d'exécution" style="width:22px;height:22px">${icon('clock', { size: 12 })}</button>
          <button class="iconbtn" data-del-auto="${a.id}" title="Supprimer" style="width:22px;height:22px">${icon('trash', { size: 12 })}</button>
        </div>
        <div data-runs-host="${a.id}" style="display:none;margin-top:6px"></div>
      </div>`;
  }).join('') || '<span class="muted" style="font-size:12.5px">Aucune règle définie. Créez-en une pour automatiser ce board — y compris vers d’autres boards.</span>';

  host.querySelectorAll('[data-toggle]').forEach((b) => {
    b.onclick = async () => {
      const a = autos.find((x) => x.id === b.dataset.toggle);
      try { await api.updateAutomation(a.id, { enabled: !a.enabled }); renderSettings(); }
      catch (e) { toast(e.message, 'err'); }
    };
  });
  host.querySelectorAll('[data-run]').forEach((b) => {
    b.onclick = async () => {
      try {
        const r = await api.runAutomation(b.dataset.run);
        toast(`${r.cards_touched} carte(s) traitée(s)`, 'ok');
        await loadBoard(boardId);
        renderSettings();
      } catch (e) { toast(e.message, 'err'); }
    };
  });
  host.querySelectorAll('[data-edit]').forEach((b) => {
    b.onclick = () => openAutomationModal(boardId, autos.find((x) => x.id === b.dataset.edit));
  });
  host.querySelectorAll('[data-del-auto]').forEach((b) => {
    b.onclick = async () => {
      try { await api.deleteAutomation(b.dataset.delAuto); renderSettings(); }
      catch (e) { toast(e.message, 'err'); }
    };
  });
  host.querySelectorAll('[data-runs]').forEach((b) => {
    b.onclick = async () => {
      const box = host.querySelector(`[data-runs-host="${b.dataset.runs}"]`);
      if (box.style.display !== 'none') { box.style.display = 'none'; return; }
      try {
        const runs = await api.automationRuns(b.dataset.runs, 10);
        box.innerHTML = runs.map((r) => `
          <div class="row" style="font-size:11px;padding:3px 6px;gap:6px">
            <span class="badge" style="background:${r.ok ? 'var(--success)' : 'var(--error)'};color:#fff">${r.ok ? 'OK' : 'ERR'}</span>
            <span class="muted" style="flex:1">${escapeHtml((r.created_at || '').replace('T', ' ').slice(0, 19))} · ${escapeHtml(r.trigger)}</span>
            <span class="muted" style="max-width:45%;overflow:hidden;text-overflow:ellipsis;white-space:nowrap" title="${escapeHtml(r.detail)}">${escapeHtml(r.detail)}</span>
          </div>`).join('') || '<span class="muted" style="font-size:11px">Aucune exécution pour l’instant.</span>';
        box.style.display = 'block';
      } catch (e) { toast(e.message, 'err'); }
    };
  });
}

// Modale de création / édition d'une règle (déclencheur → conditions → actions).
async function openAutomationModal(boardId, existing) {
  const lists = state.lists || [];
  const labels = state.labels || [];
  const fields = state.fields || [];
  const boards = state.boards?.length ? state.boards : await api.boards();
  const listsCache = { [boardId]: lists };
  await Promise.all(boards.map(async (b) => {
    if (!listsCache[b.id]) { try { listsCache[b.id] = await api.lists(b.id); } catch { listsCache[b.id] = []; } }
  }));

  let conds = {}; try { conds = JSON.parse(existing?.conditions || '{}'); } catch { conds = {}; }
  let actions = []; try { actions = JSON.parse(existing?.actions || '[]'); } catch { actions = []; }

  const listOpts = (ls, sel) => `<option value="">— toutes —</option>` + (ls || []).map((l) => `<option value="${l.id}" ${l.id === sel ? 'selected' : ''}>${escapeHtml(l.title)}</option>`).join('');
  const labelOpts = (sel) => `<option value="">— tous —</option>` + labels.map((l) => `<option value="${l.id}" ${l.id === sel ? 'selected' : ''}>${escapeHtml(l.name)}</option>`).join('');
  const fieldOpts = (sel) => `<option value="">— choisir —</option>` + fields.map((f) => `<option value="${f.id}" ${f.id === sel ? 'selected' : ''}>${escapeHtml(f.name)}</option>`).join('');
  const prioOpts = (sel) => PRIORITIES_UI.map(([v, n]) => `<option value="${v}" ${v === sel ? 'selected' : ''}>${n}</option>`).join('');
  const weekdayOpts = (sel) => WEEKDAYS_UI.map(([v, n]) => `<option value="${v}" ${v === sel ? 'selected' : ''}>${n}</option>`).join('');
  const boardOpts = (sel) => boards.map((b) => `<option value="${b.id}" ${b.id === sel ? 'selected' : ''}>${escapeHtml(b.title)}${b.id === boardId ? ' (ce board)' : ''}</option>`).join('');

  const sel = (id, v) => (v ? 'selected' : '');

  const condFields = `
    <div class="field cond" data-for="${MOST_TRIGGERS}">
      <label>Liste concernée</label>
      <select class="input" id="au-c-list">${listOpts(lists, conds.list_id)}</select>
    </div>
    <div class="field cond" data-for="card_moved">
      <label>Depuis la liste</label>
      <select class="input" id="au-c-from">${listOpts(lists, conds.from_list_id)}</select>
    </div>
    <div class="field cond" data-for="card_moved">
      <label>Vers la liste</label>
      <select class="input" id="au-c-to">${listOpts(lists, conds.to_list_id)}</select>
    </div>
    <div class="field cond" data-for="${MOST_TRIGGERS} card_labeled card_unlabeled">
      <label>Label</label>
      <select class="input" id="au-c-label">${labelOpts(conds.label_id)}</select>
    </div>
    <div class="field cond" data-for="${MOST_TRIGGERS} card_labeled card_unlabeled">
      <label>Titre contient</label>
      <input class="input" id="au-c-title" value="${escapeHtml(conds.title_contains || '')}" placeholder="ex. : facture" />
    </div>
    <div class="field cond" data-for="${MOST_TRIGGERS}">
      <label>Priorité</label>
      <select class="input" id="au-c-prio"><option value="">— toutes —</option>${prioOpts(conds.priority)}</select>
    </div>
    <div class="row cond" data-for="${MOST_TRIGGERS}" style="gap:12px">
      <label style="font-size:12.5px"><select class="input" id="au-c-hasdue" style="width:auto;display:inline-block">
        <option value="" ${conds.has_due == null ? 'selected' : ''}>échéance : indifférent</option>
        <option value="1" ${conds.has_due === true ? 'selected' : ''}>avec échéance</option>
        <option value="0" ${conds.has_due === false ? 'selected' : ''}>sans échéance</option></select></label>
      <label style="font-size:12.5px"><input type="checkbox" id="au-c-overdue" ${conds.overdue ? 'checked' : ''} /> en retard uniquement</label>
    </div>
    <div class="field cond" data-for="card_due_soon">
      <label>Échéance dans (jours)</label>
      <input type="number" class="input" id="au-c-days" value="${conds.days ?? 1}" min="0" max="365" />
    </div>
    <div class="field cond" data-for="scheduled_daily scheduled_weekly scheduled_monthly">
      <label>Heure de déclenchement</label>
      <input type="time" class="input" id="au-c-time" value="${escapeHtml(conds.time || '09:00')}" />
    </div>
    <div class="field cond" data-for="scheduled_weekly">
      <label>Jour de la semaine</label>
      <select class="input" id="au-c-weekday">${weekdayOpts(conds.weekday ?? 1)}</select>
    </div>
    <div class="field cond" data-for="scheduled_monthly">
      <label>Jour du mois</label>
      <input type="number" class="input" id="au-c-day" value="${conds.day ?? 1}" min="1" max="31" />
    </div>`;

  // ---- lignes d'actions ----
  const actionRow = (a = {}) => {
    const listId = a.list_id;
    const ownerBoard = listId ? (Object.keys(listsCache).find((bid) => (listsCache[bid] || []).some((l) => l.id === listId)) || boardId) : boardId;
    const params = actionParamsHtml(a.type || 'move', a, { boardId, listsCache, boards, labels, fields, boardOpts, listOpts, labelOpts, fieldOpts, prioOpts, ownerBoard });
    return `
      <div class="au-action" style="border:1px solid var(--border);border-radius:var(--radius-xs);padding:8px;margin-bottom:8px">
        <div class="row" style="margin-bottom:6px">
          <select class="input au-type" style="flex:1">${ACTIONS_UI.map(([v, n]) => `<option value="${v}" ${v === (a.type || 'move') ? 'selected' : ''}>${n}</option>`).join('')}</select>
          <button class="iconbtn au-del" title="Retirer cette action" style="width:24px;height:24px">${icon('close', { size: 12 })}</button>
        </div>
        <div class="au-params">${params}</div>
      </div>`;
  };

  const { el, close } = openModal(`
    <div class="dialog-header">
      <div class="dialog-badge">${icon('settings', { size: 18 })}</div>
      <div class="dialog-titles">
        <h3 class="dialog-title">${existing ? 'Modifier la règle' : 'Nouvelle règle d’automatisation'}</h3>
        <p class="dialog-desc">Quand le déclencheur se produit, les actions s’appliquent — y compris dans un autre board. Variables : {{card.title}}, {{list.title}}, {{board.title}}, {{date}}, {{label.name}}…</p>
      </div>
      <button class="iconbtn dialog-close" data-close title="Fermer (Échap)">${icon('close', { size: 14 })}</button>
    </div>
    <div class="dialog-body">
      <div class="field"><label>Nom de la règle</label>
        <input class="input" id="au-name" value="${escapeHtml(existing?.name || 'Ma règle')}" placeholder="Ex. : Créer un suivi dans le board « Suivi »" />
      </div>
      <div class="field"><label>Déclencheur</label>
        <select class="input" id="au-trigger">${TRIGGERS_UI.map(([v, n]) => `<option value="${v}" ${existing?.trigger === v ? 'selected' : ''}>${n} — <code>${v}</code></option>`).join('')}</select>
      </div>
      <div id="au-conds">${condFields}</div>
      <div class="field" style="margin-top:10px">
        <label>Actions (dans l’ordre)</label>
        <div id="au-actions-list">${actions.map((a) => actionRow(a)).join('') || actionRow()}</div>
        <button class="btn sm" id="au-add-action">${icon('plus', { size: 13 })} <span>Ajouter une action</span></button>
      </div>
      <div class="field" style="margin-top:10px">
        <label><input type="checkbox" id="au-raw" /> Édition JSON brute (conditions + actions)</label>
        <div id="au-raw-box" style="display:none;margin-top:6px">
          <textarea class="input" id="au-cond-raw" style="font-family:var(--mono);font-size:12px;min-height:44px">${escapeHtml(existing?.conditions || '{}')}</textarea>
          <textarea class="input" id="au-actions-raw" style="font-family:var(--mono);font-size:12px;min-height:60px;margin-top:6px">${escapeHtml(existing?.actions || '[]')}</textarea>
        </div>
      </div>
    </div>
    <div class="dialog-footer">
      <div class="dialog-hints"><span class="kbd">Échap</span> Annuler</div>
      <div class="dialog-btns">
        <button class="btn" data-close>Annuler</button>
        <button class="btn primary" id="au-save">${existing ? 'Enregistrer' : 'Créer la règle'}</button>
      </div>
    </div>`, { size: 'lg' });

  const refreshCondVisibility = () => {
    const t = el.querySelector('#au-trigger').value;
    el.querySelectorAll('.cond').forEach((f) => {
      f.style.display = (f.dataset.for || '').split(' ').includes(t) ? '' : 'none';
    });
  };
  el.querySelector('#au-trigger').onchange = refreshCondVisibility;
  refreshCondVisibility();

  el.querySelector('#au-raw').onchange = (e) => {
    el.querySelector('#au-raw-box').style.display = e.target.checked ? '' : 'none';
    el.querySelector('#au-conds').style.display = e.target.checked ? 'none' : '';
    el.querySelector('#au-actions-list').parentElement.style.display = e.target.checked ? 'none' : '';
  };

  const renderParams = (row) => {
    const type = row.querySelector('.au-type').value;
    // conserve les valeurs courantes
    const prev = {};
    row.querySelectorAll('[data-p]').forEach((i) => { prev[i.dataset.p] = i.type === 'checkbox' ? i.checked : i.value; });
    row.querySelector('.au-params').innerHTML = actionParamsHtml(type, prev, { boardId, listsCache, boards, labels, fields, boardOpts, listOpts, labelOpts, fieldOpts, prioOpts, ownerBoard: prev.__board || boardId });
    bindBoardList(row, listsCache, listOpts);
  };
  const bindBoardList = (row, lc, lo) => {
    const bs = row.querySelector('.au-p-board');
    if (!bs) return;
    bs.onchange = () => { row.querySelector('.au-p-list').innerHTML = lo(lc[bs.value] || []); };
  };
  const addRow = (a) => {
    const wrap = document.createElement('div');
    wrap.innerHTML = actionRow(a);
    const row = wrap.firstElementChild;
    row.querySelector('.au-type').onchange = () => renderParams(row);
    row.querySelector('.au-del').onclick = () => row.remove();
    bindBoardList(row, listsCache, listOpts);
    el.querySelector('#au-actions-list').appendChild(row);
  };
  el.querySelector('#au-add-action').onclick = () => addRow();
  el.querySelectorAll('.au-action').forEach((row) => {
    row.querySelector('.au-type').onchange = () => renderParams(row);
    row.querySelector('.au-del').onclick = () => row.remove();
    bindBoardList(row, listsCache, listOpts);
  });

  el.querySelector('#au-save').onclick = async () => {
    let conditions, acts;
    if (el.querySelector('#au-raw').checked) {
      conditions = el.querySelector('#au-cond-raw').value;
      acts = el.querySelector('#au-actions-raw').value;
    } else {
      const c = {};
      const v = (id) => el.querySelector(id)?.value;
      if (v('#au-c-list')) c.list_id = v('#au-c-list');
      if (v('#au-c-from')) c.from_list_id = v('#au-c-from');
      if (v('#au-c-to')) c.to_list_id = v('#au-c-to');
      if (v('#au-c-label')) c.label_id = v('#au-c-label');
      if (v('#au-c-title')) c.title_contains = v('#au-c-title');
      if (v('#au-c-prio')) c.priority = v('#au-c-prio');
      const hd = v('#au-c-hasdue'); if (hd !== '') c.has_due = hd === '1';
      if (el.querySelector('#au-c-overdue').checked) c.overdue = true;
      if (el.querySelector('#au-c-days')?.offsetParent) c.days = Number(v('#au-c-days'));
      if (el.querySelector('#au-c-time')?.offsetParent) c.time = v('#au-c-time');
      if (el.querySelector('#au-c-weekday')?.offsetParent) c.weekday = Number(v('#au-c-weekday'));
      if (el.querySelector('#au-c-day')?.offsetParent) c.day = Number(v('#au-c-day'));
      conditions = JSON.stringify(c);
      acts = JSON.stringify([...el.querySelectorAll('.au-action')].map((row) => {
        const type = row.querySelector('.au-type').value;
        const a = { type };
        row.querySelectorAll('[data-p]').forEach((i) => {
          if (i.dataset.p === '__board') return;
          if (i.type === 'checkbox') return;
          if (i.value !== '' && i.value != null) a[i.dataset.p] = (i.dataset.num === '1') ? Number(i.value) : i.value;
        });
        if (a.position === '') delete a.position;
        const keep = {};
        row.querySelectorAll('[data-keep]').forEach((cb) => { keep[cb.dataset.keep] = cb.checked; });
        if (Object.keys(keep).length) a.keep = keep;
        return a;
      }));
    }
    try {
      const payload = { name: el.querySelector('#au-name').value, trigger: el.querySelector('#au-trigger').value, conditions, actions: acts };
      if (existing) await api.updateAutomation(existing.id, payload);
      else await api.createAutomation(boardId, payload);
      close();
      renderSettings();
    } catch (e) { toast(e.message, 'err'); }
  };
}

// Champs de paramètres d'une action, selon son type.
function actionParamsHtml(type, val, ctx) {
  const { boardId, listsCache, boards, labels, fields, boardOpts, listOpts, labelOpts, fieldOpts, prioOpts, ownerBoard } = ctx;
  const boardList = `
    <div class="row" style="gap:6px">
      <select class="input au-p-board" data-p="__board" style="flex:1">${boardOpts(ownerBoard || val.__board || boardId)}</select>
      <select class="input au-p-list" data-p="list_id" style="flex:1">${listOpts(listsCache[ownerBoard || boardId] || [], val.list_id)}</select>
    </div>`;
  const tpl = (ph) => `<span class="muted" style="font-size:10.5px">Variables : {{card.title}} {{list.title}} {{board.title}} {{date}}</span>`;
  switch (type) {
    case 'move':
      return `${boardList}
        <div class="field" style="margin-top:6px"><label>Position</label>
          <select class="input" data-p="position"><option value="">fin de la liste</option><option value="top" ${val.position === 'top' ? 'selected' : ''}>en haut</option></select></div>`;
    case 'copy':
      return `${boardList}
        <div class="field" style="margin-top:6px"><label>Titre de la copie (optionnel)</label>
          <input class="input" data-p="title" value="${escapeHtml(val.title || '')}" placeholder="{{card.title}} (copie)" />${tpl()}</div>
        <div class="row" style="gap:10px;margin-top:6px;font-size:12px">
          <label><input type="checkbox" data-keep="labels" ${val.keep?.labels !== false ? 'checked' : ''} /> labels</label>
          <label><input type="checkbox" data-keep="checklists" ${val.keep?.checklists !== false ? 'checked' : ''} /> checklists</label>
          <label><input type="checkbox" data-keep="description" ${val.keep?.description !== false ? 'checked' : ''} /> description</label>
        </div>
        <span class="muted" style="font-size:10.5px">Les dates ne sont jamais copiées. En cross-board, les labels sont recréés par nom sur le board cible.</span>`;
    case 'create_card':
      return `${boardList}
        <div class="field" style="margin-top:6px"><label>Titre de la nouvelle carte</label>
          <input class="input" data-p="title" value="${escapeHtml(val.title || '')}" placeholder="Suivi de {{card.title}}" />${tpl()}</div>
        <div class="field" style="margin-top:6px"><label>Description (optionnel)</label>
          <input class="input" data-p="description" value="${escapeHtml(val.description || '')}" /></div>
        <div class="field" style="margin-top:6px"><label>Priorité</label>
          <select class="input" data-p="priority">${prioOpts(val.priority || 'none')}</select></div>`;
    case 'label':
    case 'unlabel':
      return `<div class="field"><label>Label (board courant)</label><select class="input" data-p="label_id">${labelOpts(val.label_id)}</select></div>`;
    case 'due':
      return `<div class="row" style="gap:6px">
          <div class="field" style="flex:1"><label>Dans (jours)</label><input type="number" class="input" data-p="offset_days" data-num="1" value="${val.offset_days ?? 0}" min="-365" max="365" /></div>
          <div class="field" style="flex:1"><label>Heure</label><input type="time" class="input" data-p="time" value="${escapeHtml(val.time || '')}" /></div>
          <div class="field" style="flex:1"><label>Ou date fixe</label><input type="date" class="input" data-p="date" value="${escapeHtml(val.date || '')}" /></div>
        </div>`;
    case 'priority':
      return `<div class="field"><label>Nouvelle priorité</label><select class="input" data-p="value">${prioOpts(val.value || 'high')}</select></div>`;
    case 'check':
    case 'check_item':
      return `<div class="field"><label>Élément</label><input class="input" data-p="item_title" value="${escapeHtml(val.item_title || '')}" placeholder="Titre de l’élément" /></div>`;
    case 'comment':
      return `<div class="field"><label>Texte du commentaire</label><input class="input" data-p="text" value="${escapeHtml(val.text || '')}" placeholder="Traité automatiquement le {{date}}" />${tpl()}</div>`;
    case 'set_title':
      return `<div class="field"><label>Nouveau titre</label><input class="input" data-p="value" value="${escapeHtml(val.value || '')}" placeholder="[Fait] {{card.title}}" />${tpl()}</div>`;
    case 'set_field':
      return `<div class="row" style="gap:6px">
          <div class="field" style="flex:1"><label>Champ (board courant)</label><select class="input" data-p="field_id">${fieldOpts(val.field_id)}</select></div>
          <div class="field" style="flex:1"><label>Valeur</label><input class="input" data-p="value" value="${escapeHtml(val.value ?? '')}" /></div>
        </div>`;
    default:
      return '<span class="muted" style="font-size:12px">Aucun paramètre.</span>';
  }
}

export function renderSettings() {
  const host = document.querySelector('.view');
  const board = state.board;

  host.innerHTML = `
    <div class="view-head" style="margin-bottom:18px">
      <h2 style="margin:0;font-size:20px;font-weight:700">Paramètres</h2>
      <p class="muted" style="margin:4px 0 0;font-size:13px">Préférences d'affichage et configuration ${board ? `du board « ${escapeHtml(board.title)} »` : 'générale'}.</p>
    </div>
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
          ${icon('layout', { size: 16, className: 'muted' })}
          <span>Style d'interface</span>
        </h3>
        <p class="muted" style="margin:0 0 12px;font-size:12.5px">Basculez entre le design Standard et le design Translucide (Liquid Glass / visionOS).</p>
        <div class="viewtabs" id="st-style-picker" style="width:100%;display:flex">
          <button data-style="standard" class="${state.style === 'standard' ? 'active' : ''}" style="flex:1;justify-content:center">
            ${icon('kanban', { size: 14 })} <span>Standard</span>
          </button>
          <button data-style="translucent" class="${state.style === 'translucent' ? 'active' : ''}" style="flex:1;justify-content:center">
            ${icon('ai', { size: 14 })} <span>Translucide Liquid Glass</span>
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

      <div class="panel" style="border:1px solid var(--error)">
        <h3 style="display:flex;align-items:center;gap:8px;color:var(--error)">
          ${icon('alert', { size: 16 })}
          <span>Zone de danger</span>
        </h3>
        <p class="muted" style="margin:0 0 12px;font-size:12.5px">
          Suppressions définitives de données locales. Annulables uniquement via ⌘Z pendant la session en cours.
        </p>
        <div class="row" style="flex-wrap:wrap;gap:8px">
          ${board ? `<button class="btn danger sm" id="st-board-delete2">${icon('trash', { size: 13 })} <span>Supprimer ce board</span></button>` : ''}
          <button class="btn danger sm" id="st-delete-all">${icon('trash', { size: 13 })} <span>Tout supprimer — tous les boards</span></button>
        </div>
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
        <h3 style="display:flex;align-items:center;gap:8px">
          ${icon('settings', { size: 16, className: 'muted' })}
          <span>Automatisations (bot local)</span>
        </h3>
        <p class="muted" style="margin:0 0 10px;font-size:12.5px">Règles « quand… alors… » façon Butler. Les actions peuvent cibler un autre board (copier / créer / déplacer). Chaque exécution est journalisée.</p>
        <div id="st-automations"></div>
        <button class="btn sm" id="st-add-automation" style="margin-top:10px">
          ${icon('plus', { size: 13 })} <span>Nouvelle règle</span>
        </button>
        <button class="btn sm" id="st-auto-runs-toggle" style="margin-top:10px">
          ${icon('clock', { size: 13 })} <span>Journal d’exécution du board</span>
        </button>
        <div id="st-auto-runs" style="display:none;margin-top:8px"></div>
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
    const sb = document.getElementById('sidebar');
    if (sb) {
      import('../components/sidebar.js').then((m) => m.renderSidebar(sb));
    }
    renderSettings();
  });

  // --- Style (Standard vs Translucide Liquid Glass)
  host.querySelector('#st-style-picker')?.addEventListener('click', (e) => {
    const btn = e.target.closest('[data-style]');
    if (!btn) return;
    state.style = btn.dataset.style;
    applyStyle();
    const tb = document.getElementById('topbar');
    if (tb) {
      import('../components/topbar.js').then((m) => m.renderTopbar(tb));
    }
    const sb = document.getElementById('sidebar');
    if (sb) {
      import('../components/sidebar.js').then((m) => m.renderSidebar(sb));
    }
    renderSettings();
  });

  // --- IA
  api.aiStatus().then((s) => {
    host.querySelector('#st-ai').innerHTML = s.available
      ? `<span class="badge done">● Disponible</span> <p class="muted" style="margin:8px 0 0;font-size:12.5px">Moteur Apple Intelligence actif (${s.backend}). 100 % on-device — aucune donnée ne quitte votre appareil.</p>`
      : `<span class="badge overdue">● Indisponible</span> <p class="muted" style="margin:8px 0 0;font-size:12.5px">${s.reason || ''} — Nécessite macOS 15.1+ sur Mac Apple Silicon avec Apple Intelligence activée dans Réglages Système.</p>`;
  });

  // --- zone de danger globale : tout supprimer (disponible même sans board ouvert)
  host.querySelector('#st-delete-all').onclick = async () => {
    const n = state.boards.length;
    if (n === 0) { toast('Aucun board à supprimer', 'err'); return; }
    if (!(await confirmDialog(`⚠ TOUT SUPPRIMER : supprimer définitivement les ${n} board${n > 1 ? 's' : ''} (listes, cartes, labels, champs, règles, commentaires, pièces jointes) ? Annulable uniquement avec ⌘Z pendant cette session.`))) return;
    try {
      const r = await api.deleteAllBoards();
      await loadBoards();
      state.board = null;
      state.lists = []; state.cards = []; state.labels = []; state.fields = []; state.deps = []; state.schedule = null;
      applyBoardBackground(null);
      toast(`${r.deleted} board(s) supprimé(s)`, 'ok');
      navigate('#/');
      renderSettings();
    } catch (e) { toast(e.message, 'err'); }
  };

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
      const sb = document.getElementById('sidebar');
      if (sb) {
        import('../components/sidebar.js').then((m) => m.renderSidebar(sb));
      }
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
  const delBoardCurrent = async () => {
    if (!(await confirmDialog(`Supprimer définitivement le board « ${board.title} » ? Toutes ses listes, cartes, labels et règles seront supprimées (annulable avec ⌘Z dans la session).`))) return;
    try {
      await api.deleteBoard(boardId);
      await loadBoards();
      toast(`Board « ${board.title} » supprimé`, 'ok');
      if (state.boards.length) navigate(boardUrl(state.boards[0].id, 'board'));
      else { state.board = null; applyBoardBackground(null); navigate('#/'); }
    } catch (e) { toast(e.message, 'err'); }
  };
  host.querySelector('#st-board-delete').onclick = delBoardCurrent;
  host.querySelector('#st-board-delete2')?.addEventListener('click', delBoardCurrent);

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
  renderAutomations(host.querySelector('#st-automations'), boardId);
  host.querySelector('#st-add-automation').onclick = () => openAutomationModal(boardId, null);
  host.querySelector('#st-auto-runs-toggle').onclick = async () => {
    const box = host.querySelector('#st-auto-runs');
    if (box.style.display !== 'none') { box.style.display = 'none'; return; }
    try {
      const autos = await api.automations(boardId);
      const runs = await api.boardAutomationRuns(boardId, 30);
      const nameOf = (id) => autos.find((a) => a.id === id)?.name || id;
      box.innerHTML = runs.map((r) => `
        <div class="row" style="font-size:11.5px;padding:4px 8px;gap:8px;background:var(--surface-2);border-radius:var(--radius-xs);margin-bottom:4px">
          <span class="badge" style="background:${r.ok ? 'var(--success)' : 'var(--error)'};color:#fff">${r.ok ? 'OK' : 'ERR'}</span>
          <b style="white-space:nowrap">${escapeHtml(nameOf(r.automation_id))}</b>
          <span class="muted" style="flex:1">${escapeHtml(r.trigger)} · ${escapeHtml((r.created_at || '').replace('T', ' ').slice(0, 19))}</span>
          <span class="muted" style="max-width:35%;overflow:hidden;text-overflow:ellipsis;white-space:nowrap" title="${escapeHtml(r.detail)}">${escapeHtml(r.detail)}</span>
        </div>`).join('') || '<span class="muted" style="font-size:12px">Aucune exécution journalisée.</span>';
      box.style.display = 'block';
    } catch (e) { toast(e.message, 'err'); }
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
