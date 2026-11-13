// Vue Kanban : listes + cartes, drag & drop, navigation clavier power-user, quick add, WIP.

import { api } from '../api.js';
import { escapeHtml, PRIORITY_META } from '../format.js';
import { state, cardsOfList, visibleLists, refreshBoard } from '../state.js';
import { draggable, activeDrag, beginDrag, lastPointerY } from '../dnd.js';
import { toast, promptDialog, confirmDialog, openModal } from '../ui/modal.js';
import { cardTileHtml, updateCardTileInDOM } from '../components/card_tile.js';
import { openCardModal } from '../components/card_modal.js';
import { saveViewSettings } from './view_helpers.js';
import { icon } from '../icons.js';

let host = null;
let focusedCardId = null;
let keyNavAttached = false;
let selection = new Set();
let selEscAttached = false;

export function renderBoard() {
  host = document.querySelector('.view');
  host.innerHTML = `
    <div class="filters" id="board-filters"></div>
    <div class="board-scroller">
      <div class="board" id="board"></div>
    </div>`;
  renderFilters();
  renderLists();
  initBoardKeyNav();
  if (!selEscAttached) {
    selEscAttached = true;
    window.addEventListener('keydown', (e) => {
      if (e.key === 'Escape' && selection.size) clearSelection();
    });
  }
  if (state.style === 'translucent') {
    renderTranslucentCopilot();
  } else {
    document.querySelector('.translucent-copilot-card')?.remove();
  }
}

function renderTranslucentCopilot() {
  document.querySelector('.translucent-copilot-card')?.remove();
  if (sessionStorage.getItem('krello-hide-copilot-widget') === '1') return;

  const widget = document.createElement('div');
  widget.className = 'translucent-copilot-card fade-in';
  widget.innerHTML = `
    <div class="copilot-card-icon">${icon('ai', { size: 18 })}</div>
    <div class="copilot-card-content">
      <div class="copilot-card-title">Assistant Krello</div>
      <div class="copilot-card-desc">Ordonnancement à jour • Prêt à vous assister</div>
    </div>
    <button class="iconbtn copilot-card-close" title="Fermer" style="width:20px;height:20px;margin-left:auto">
      ${icon('close', { size: 12 })}
    </button>
  `;

  widget.querySelector('.copilot-card-close').onclick = (e) => {
    e.stopPropagation();
    sessionStorage.setItem('krello-hide-copilot-widget', '1');
    widget.remove();
  };

  widget.onclick = () => {
    import('../components/ai_panel.js').then((m) => m.initAiDrawer());
  };

  document.body.appendChild(widget);
}

function renderFilters() {
  const bar = host.querySelector('#board-filters');
  const f = state.filters;
  const visible = visibleLists().reduce((s, l) => s + cardsOfList(l.id).length, 0);
  const total = state.cards.filter((c) => !c.archived && !c.is_template).length;
  const hasFilters = f.text || f.labelId || f.priority || f.due || f.includeArchived;
  const currentSavedView = bar?.querySelector('#f-saved')?.value || '';
  const savedViews = Object.keys(savedFiltersMap());

  bar.innerHTML = `
    <span class="flabel" title="Filtres">${icon('filter', { size: 13 })} <span class="flabel-text">Filtres</span></span>
    <input class="input" id="f-text" placeholder="Rechercher…" title="Rechercher par titre ou description" value="${escapeHtml(f.text)}" />
    <select class="input" id="f-label" title="Filtrer par étiquette">
      <option value="">Labels</option>
      ${state.labels.map((l) => `<option value="${l.id}" ${f.labelId === l.id ? 'selected' : ''}>${escapeHtml(l.name || '•')}</option>`).join('')}
    </select>
    <select class="input" id="f-priority" title="Filtrer par priorité">
      <option value="">Priorités</option>
      ${Object.entries(PRIORITY_META).filter(([k]) => k !== 'none')
        .map(([k, v]) => `<option value="${k}" ${f.priority === k ? 'selected' : ''}>${v.label}</option>`).join('')}
    </select>
    <select class="input" id="f-due" title="Filtrer par échéance">
      <option value="">Échéances</option>
      <option value="overdue" ${f.due === 'overdue' ? 'selected' : ''}>En retard</option>
      <option value="week" ${f.due === 'week' ? 'selected' : ''}>≤ 7 jours</option>
    </select>
    <label class="f-archived-label" title="Afficher les cartes archivées">
      <input type="checkbox" id="f-archived" ${f.includeArchived ? 'checked' : ''} style="width:13px;height:13px;cursor:pointer" />
      <span class="flabel-text">Archivés</span>
    </label>
    ${hasFilters ? `<button class="iconbtn" id="f-reset" title="Réinitialiser les filtres" style="width:24px;height:24px;flex:none">${icon('close', { size: 12 })}</button>` : ''}
    <span class="fcount" title="${visible} carte(s) visible(s) sur ${total}">${visible}/${total}<span class="fcount-label"> cartes</span></span>
    <select class="input" id="f-saved" title="Vues de filtres enregistrées">
      <option value="">Vues…</option>
      ${savedViews.map((n) => `<option value="${escapeHtml(n)}" ${currentSavedView === n ? 'selected' : ''}>${escapeHtml(n)}</option>`).join('')}
    </select>
    <button class="iconbtn" id="f-save-view" title="Enregistrer la vue courante">${icon('check', { size: 12 })}</button>
    <button class="iconbtn" id="f-del-view" title="Supprimer la vue sélectionnée" style="${currentSavedView ? '' : 'display:none'}">${icon('trash', { size: 12 })}</button>
    <button class="iconbtn" id="f-help" title="Raccourcis clavier (?)">
      <span class="kbd" style="font-size:10.5px">?</span>
    </button>
    <button class="iconbtn" id="f-board-settings" title="Paramètres du tableau (Ambiance, planning, labels, champs, règles…)">
      ${icon('settings', { size: 13 })}
    </button>`;

  let filterTimer = null;
  bar.querySelector('#f-text').addEventListener('input', (e) => {
    state.filters.text = e.target.value;
    clearTimeout(filterTimer);
    // Debounce : renderLists reconstruit toutes les tuiles, inutile à chaque frappe.
    // (#f-text vit dans #board-filters, hors de #board : jamais détruit, focus conservé.)
    filterTimer = setTimeout(() => renderLists(), 120);
  });
  bar.querySelector('#f-label').addEventListener('change', (e) => { state.filters.labelId = e.target.value || null; renderLists(); });
  bar.querySelector('#f-priority').addEventListener('change', (e) => { state.filters.priority = e.target.value || null; renderLists(); });
  bar.querySelector('#f-due').addEventListener('change', (e) => { state.filters.due = e.target.value || null; renderLists(); });
  bar.querySelector('#f-archived').addEventListener('change', (e) => { state.filters.includeArchived = e.target.checked; renderFilters(); renderLists(); });
  bar.querySelector('#f-reset')?.addEventListener('click', () => {
    state.filters = { text: '', labelId: null, priority: null, due: null, includeArchived: false };
    renderFilters();
    renderLists();
  });
  bar.querySelector('#f-help')?.addEventListener('click', () => openShortcutsDialog());
  bar.querySelector('#f-board-settings')?.addEventListener('click', () => {
    if (state.board) {
      import('../components/board_settings_modal.js').then((m) => m.openBoardSettingsModal(state.board.id));
    }
  });

  // Vues de filtres enregistrées (persistées dans les réglages du board).
  bar.querySelector('#f-saved')?.addEventListener('change', (e) => {
    const val = e.target.value;
    const delBtn = bar.querySelector('#f-del-view');
    if (delBtn) delBtn.style.display = val ? 'inline-grid' : 'none';
    const f = savedFiltersMap()[val];
    if (!f) return;
    state.filters = {
      text: f.text || '',
      labelId: f.labelId || null,
      priority: f.priority || null,
      due: f.due || null,
      includeArchived: !!f.includeArchived,
    };
    renderFilters();
    renderLists();
  });
  bar.querySelector('#f-save-view')?.addEventListener('click', async () => {
    const name = await promptDialog('Nom de la vue enregistrée', 'Ma vue');
    if (!name) return;
    const map = savedFiltersMap();
    map[name] = { ...state.filters };
    try {
      await saveViewSettings('board', { savedFilters: JSON.stringify(map) });
      toast(`Vue « ${name} » enregistrée`, 'ok');
      renderFilters();
      const savedSel = bar.querySelector('#f-saved');
      if (savedSel) {
        savedSel.value = name;
        const delBtn = bar.querySelector('#f-del-view');
        if (delBtn) delBtn.style.display = 'inline-grid';
      }
    } catch (e) { toast(e.message, 'err'); }
  });
  bar.querySelector('#f-del-view')?.addEventListener('click', async () => {
    const name = bar.querySelector('#f-saved')?.value;
    if (!name) { toast('Sélectionnez d\'abord une vue dans la liste', 'err'); return; }
    if (!(await confirmDialog(`Supprimer la vue « ${name} » ?`))) return;
    const map = savedFiltersMap();
    delete map[name];
    try {
      await saveViewSettings('board', { savedFilters: JSON.stringify(map) });
      toast('Vue supprimée', 'ok');
      renderFilters();
    } catch (e) { toast(e.message, 'err'); }
  });
}

function savedFiltersMap() {
  const raw = state.settings?.board?.savedFilters;
  if (!raw) return {};
  try { return JSON.parse(raw); } catch { return {}; }
}

function renderLists() {
  const board = host?.querySelector('#board');
  if (!board) return;
  const listScrolls = new Map();
  board.querySelectorAll('.list-body').forEach((lb) => {
    listScrolls.set(lb.dataset.body, lb.scrollTop);
  });

  board.innerHTML = '';

  for (const list of visibleLists()) {
    const el = listEl(list);
    board.appendChild(el);
    const prevScroll = listScrolls.get(list.id);
    if (prevScroll) {
      const lb = el.querySelector('.list-body');
      if (lb) lb.scrollTop = prevScroll;
    }
  }

  const addList = document.createElement('button');
  addList.className = 'list-add';
  addList.innerHTML = `
    <div style="display:flex;align-items:center;justify-content:center;gap:8px">
      ${icon('plus', { size: 16 })}
      <span>Ajouter une liste</span>
    </div>
  `;
  addList.onclick = async () => {
    const title = await promptDialog('Nom de la liste', '', 'Ex. : En cours, Revue, Bloqué');
    if (!title) return;
    try {
      await api.createList(state.board.id, { title });
      await reload();
    } catch (e) { toast(e.message, 'err'); }
  };
  board.appendChild(addList);
  updateFilterBar();
  renderSelectionBar();
  applyFocusVisual();
}

/** Met à jour compteur + bouton reset sans reconstruire la barre (focus conservé). */
function updateFilterBar() {
  const bar = host.querySelector('#board-filters');
  if (!bar) return;
  const f = state.filters;
  const visible = visibleLists().reduce((s, l) => s + cardsOfList(l.id).length, 0);
  const total = state.cards.filter((c) => !c.archived && !c.is_template).length;
  const count = bar.querySelector('.fcount');
  if (count) {
    count.title = `${visible} carte(s) visible(s) sur ${total}`;
    count.innerHTML = `${visible}/${total}<span class="fcount-label"> cartes</span>`;
  }
  const hasFilters = !!(f.text || f.labelId || f.priority || f.due || f.includeArchived);
  let reset = bar.querySelector('#f-reset');
  if (hasFilters && !reset) {
    reset = document.createElement('button');
    reset.className = 'iconbtn';
    reset.id = 'f-reset';
    reset.title = 'Réinitialiser les filtres';
    reset.style.cssText = 'width:24px;height:24px;flex:none';
    reset.innerHTML = icon('close', { size: 12 });
    reset.addEventListener('click', () => {
      state.filters = { text: '', labelId: null, priority: null, due: null, includeArchived: false };
      renderFilters();
      renderLists();
    });
    count?.before(reset);
  } else if (!hasFilters && reset) {
    reset.remove();
  }
}

function listEl(list) {
  const el = document.createElement('div');
  el.className = 'list';
  el.dataset.list = list.id;
  const cards = cardsOfList(list.id);
  const over = list.wip_limit && cards.length > list.wip_limit;

  // Ordre arbre : parents d'abord, sous-cartes indentées sous leur parent.
  const ids = new Set(cards.map((c) => c.id));
  const kidsOf = new Map();
  const tops = [];
  for (const c of cards) {
    if (c.parent_id && ids.has(c.parent_id)) {
      if (!kidsOf.has(c.parent_id)) kidsOf.set(c.parent_id, []);
      kidsOf.get(c.parent_id).push(c);
    } else {
      tops.push(c);
    }
  }
  const rows = [];
  const pushTree = (group, depth) => {
    for (const c of group) {
      const kids = kidsOf.get(c.id);
      rows.push({
        c,
        depth,
        subLabel: kids ? `${kids.filter((k) => k.due_complete).length}/${kids.length} sous-cartes` : null,
      });
      if (kids) pushTree(kids, depth + 1);
    }
  };
  pushTree(tops, 0);

  el.innerHTML = `
    <div class="list-head">
      <span class="list-status-dot"></span>
      <h3 data-edit-list="${list.id}" title="Cliquez pour renommer">${escapeHtml(list.title)}</h3>
      <span class="count ${over ? 'wip-warn' : ''}" title="${list.wip_limit ? `Limite WIP: ${list.wip_limit}` : ''}">
        ${cards.length}${list.wip_limit ? `/${list.wip_limit}` : ''}
      </span>
      <button class="iconbtn" data-list-menu="${list.id}" title="Menu liste" style="width:26px;height:26px">
        ${icon('more', { size: 15 })}
      </button>
    </div>
    <div class="list-body" data-body="${list.id}">
      ${rows.map(({ c, depth, subLabel }) => cardTileHtml(c, { selected: selection.has(c.id), depth, subLabel })).join('')}
    </div>
    <div class="list-foot">
      <button class="add-card" data-add-card="${list.id}">
        ${icon('plus', { size: 13 })}
        <span>Ajouter une carte</span>
      </button>
      <button class="iconbtn" data-add-from-template="${list.id}" title="Créer depuis un modèle">
        ${icon('template', { size: 14 })}
      </button>
    </div>
  `;

  // Les interactions des tuiles (clic, drag, drop fichiers) sont déléguées sur
  // document (voir installTileDelegation ci-dessous) : rien à attacher par tuile.

  // quick add
  el.querySelector('[data-add-card]').onclick = () => quickAdd(list.id, el.querySelector('.list-body'));

  // galerie de modèles
  el.querySelector('[data-add-from-template]')?.addEventListener('click', () => openTemplateGallery(list.id));

  // édition titre + menu
  el.querySelector('[data-edit-list]').addEventListener('click', async () => {
    const nv = await promptDialog('Renommer la liste', list.title);
    if (nv && nv !== list.title) { await api.updateList(list.id, { title: nv }); reload(); }
  });
  el.querySelector('[data-list-menu]').addEventListener('click', async (e) => {
    showListMenu(e.currentTarget, list);
  });

  // drag de la liste (poignée = en-tête)
  draggable(el, {
    kind: 'list',
    id: list.id,
    canDrag: (ev) => !!ev.target.closest('.list-head h3, .list-head') && !ev.target.closest('button'),
    targetSelector: '.board .list',
    onDrop: async ({ targetEl }) => {
      const targetId = targetEl.dataset.list;
      if (!targetId || targetId === list.id) return;
      const boardEl = document.querySelector('#board');
      const moving = boardEl.querySelector(`.list[data-list="${list.id}"]`);
      boardEl.insertBefore(moving, targetEl);
      // calcule positions : ordre d'affichage
      const ids = [...boardEl.querySelectorAll('.list')].map((n) => n.dataset.list);
      for (let i = 0; i < ids.length; i++) {
        const l = visibleLists().find((x) => x.id === ids[i]);
        if (l && l.position !== i) await api.updateList(l.id, { position: i });
      }
      reload();
    },
  });

  return el;
}

// --- sélection multiple (⌘/⇧+clic) et actions en masse
function boardTileClick(ev, card, tile) {
  if (ev.metaKey || ev.ctrlKey || ev.shiftKey) {
    ev.preventDefault();
    ev.stopPropagation();
    if (selection.has(card.id)) {
      selection.delete(card.id);
      tile.classList.remove('selected');
    } else {
      selection.add(card.id);
      tile.classList.add('selected');
    }
    renderSelectionBar();
    return;
  }
  if (selection.size) {
    selection.clear();
    renderLists();
  }
  focusedCardId = card.id;
  applyFocusVisual();
  openCardModal(card.id, { onChange: reload });
}

// --- Délégation des événements des tuiles de carte ---
// Un seul jeu de listeners sur document (posé une fois à l'import du module),
// au lieu de 5 listeners × N tuiles recréés à chaque renderLists().
function boardTileFromEvent(ev) {
  const tile = ev.target.closest?.('.card-tile');
  if (!tile || !tile.closest('#board .list')) return null;
  const card = state.cards.find((c) => c.id === tile.dataset.card);
  return card ? { tile, card } : null;
}

function cardDragOpts(card) {
  return {
    kind: 'card',
    id: card.id,
    targetSelector: '.list',
    canDrag: (ev) => !ev.target.closest('button, input, textarea, a'),
    onDrop: async ({ targetEl }) => {
      const listEl = targetEl.closest('.list');
      const targetListId = listEl?.dataset.list;
      if (!targetListId) return;
      // position : carte avant laquelle insérer (position du pointeur)
      const byId = new Map(state.cards.map((c) => [c.id, c]));
      const isDesc = (c) => {
        let p = c.parent_id;
        while (p) {
          if (p === card.id) return true;
          p = byId.get(p)?.parent_id || null;
        }
        return false;
      };
      const tiles = [...listEl.querySelectorAll('.card-tile')]
        .filter((x) => x.dataset.card !== card.id)
        .filter((x) => {
          const cx = byId.get(x.dataset.card);
          return cx && !isDesc(cx);
        });
      const beforeTile = tiles.find((x) => x.getBoundingClientRect().bottom > lastPointerY());
      const beforeId = beforeTile?.dataset.card || null;
      // Niveau d'imbrication : celui de la carte de référence (même niveau),
      // top-level en fin de liste. Changer de parent passe par la modale.
      const beforeCard = beforeId ? byId.get(beforeId) : null;
      const parentId = beforeCard ? (beforeCard.parent_id || '') : '';
      try {
        await window.__krelloMoveCard(card.id, { list_id: targetListId, before: beforeId || undefined, parent_id: parentId });
        reload();
      } catch (e) {
        toast(e.message, 'err');
      }
    },
  };
}

document.addEventListener('pointerdown', (ev) => {
  if (ev.button !== 0) return;
  const t = boardTileFromEvent(ev);
  if (!t) return;
  if (ev.target.closest('button, input, textarea, a')) return;
  beginDrag(t.tile, cardDragOpts(t.card), ev);
});

document.addEventListener('click', (ev) => {
  const t = boardTileFromEvent(ev);
  if (!t) return;
  boardTileClick(ev, t.card, t.tile);
});

// Pièces jointes par glisser-déposer de fichiers externes (bureau / explorateur).
// Le DnD interne utilise Pointer Events : pas de conflit avec le drag HTML5 des fichiers.
document.addEventListener('dragover', (ev) => {
  const t = boardTileFromEvent(ev);
  if (!t) return;
  if (![...ev.dataTransfer?.types || []].includes('Files')) return;
  ev.preventDefault();
  ev.dataTransfer.dropEffect = 'copy';
  t.tile.classList.add('file-over');
});
document.addEventListener('dragleave', (ev) => {
  const t = boardTileFromEvent(ev);
  if (!t) return;
  t.tile.classList.remove('file-over');
});
document.addEventListener('drop', async (ev) => {
  const t = boardTileFromEvent(ev);
  if (!t) return;
  if (![...(ev.dataTransfer?.types || [])].includes('Files')) return;
  ev.preventDefault();
  t.tile.classList.remove('file-over');
  const files = [...ev.dataTransfer.files];
  if (!files.length) return;
  let ok = 0, fail = 0;
  for (const f of files) {
    try { await api.uploadAttachment(t.card.id, f); ok++; }
    catch (err) { fail++; toast(err.message, 'err'); }
  }
  if (ok) toast(`${ok} pièce(s) jointe(s) ajoutée(s) à « ${t.card.title.slice(0, 30)} »`, 'ok');
  reload();
});

function renderSelectionBar() {
  document.getElementById('selection-bar')?.remove();
  if (!selection.size) return;
  const bar = document.createElement('div');
  bar.id = 'selection-bar';
  bar.style.cssText = 'display:flex;align-items:center;gap:8px;flex-wrap:wrap;padding:8px 12px;margin:0 0 10px;border-radius:var(--radius-sm);background:var(--accent-soft);border:1px solid color-mix(in srgb,var(--accent)35%,transparent)';
  bar.innerHTML = `
    <span class="badge" style="color:var(--accent)">${icon('kanban', { size: 12 })} ${selection.size} sélectionnée${selection.size > 1 ? 's' : ''}</span>
    <button class="btn sm" data-sel="move">Déplacer vers…</button>
    <button class="btn sm" data-sel="label">Label…</button>
    <button class="btn sm" data-sel="archive">Archiver</button>
    <button class="btn sm danger" data-sel="delete">Supprimer</button>
    <button class="iconbtn" data-sel="clear" title="Vider la sélection (Échap)" style="width:24px;height:24px">${icon('close', { size: 12 })}</button>`;
  host.querySelector('#board-filters')?.after(bar);
  bar.querySelector('[data-sel=archive]').onclick = () =>
    bulkRun((id) => api.updateCard(id, { archived: true }), 'Archivées');
  bar.querySelector('[data-sel=delete]').onclick = async () => {
    if (!(await confirmDialog(`Supprimer définitivement ${selection.size} carte(s) ?`))) return;
    bulkRun((id) => api.deleteCard(id), 'Supprimées');
  };
  bar.querySelector('[data-sel=move]').onclick = bulkPickList;
  bar.querySelector('[data-sel=label]').onclick = bulkPickLabel;
  bar.querySelector('[data-sel=clear]').onclick = () => clearSelection();
}

async function bulkRun(fn, done) {
  let ok = 0, fail = 0;
  for (const id of selection) {
    try { await fn(id); ok++; } catch { fail++; }
  }
  toast(`${done} : ${ok} carte(s)${fail ? ` · ${fail} échec(s)` : ''}`, fail ? 'err' : 'ok');
  selection.clear();
  await reload();
}

async function bulkPickList() {
  const lists = visibleLists();
  if (!lists.length) return;
  const { el, close } = openModal(`
    <div class="modal-head"><h2>Déplacer ${selection.size} carte(s)</h2>
      <button class="iconbtn" data-close title="Fermer">${icon('close', { size: 12 })}</button></div>
    <div class="modal-body">
      <div class="field"><label>Liste de destination (fin de liste)</label>
        <select class="input" id="sel-move-list">
          ${lists.map((l) => `<option value="${l.id}">${escapeHtml(l.title)}</option>`).join('')}
        </select></div>
      <div class="row" style="margin-top:12px;justify-content:flex-end">
        <button class="btn primary sm" id="sel-move-go">Déplacer</button></div>
    </div>`, { id: 'bulk-move' });
  el.querySelector('#sel-move-go').onclick = async () => {
    const listId = el.querySelector('#sel-move-list').value;
    close();
    await bulkRun((id) => api.moveCard(id, { list_id: listId }), 'Déplacées');
  };
}

async function bulkPickLabel() {
  if (!state.labels.length) {
    toast('Aucun label sur ce board — créez-en un dans Paramètres', 'err');
    return;
  }
  const { el, close } = openModal(`
    <div class="modal-head"><h2>Appliquer un label à ${selection.size} carte(s)</h2>
      <button class="iconbtn" data-close title="Fermer">${icon('close', { size: 12 })}</button></div>
    <div class="modal-body">
      <div class="field"><label>Label</label>
        <select class="input" id="sel-label">
          ${state.labels.map((l) => `<option value="${l.id}">${escapeHtml(l.name || '•')}</option>`).join('')}
        </select></div>
      <div class="row" style="margin-top:12px;justify-content:flex-end">
        <button class="btn primary sm" id="sel-label-go">Appliquer</button></div>
    </div>`, { id: 'bulk-label' });
  el.querySelector('#sel-label-go').onclick = async () => {
    const labelId = el.querySelector('#sel-label').value;
    close();
    await bulkRun((id) => api.attachLabel(id, labelId), 'Étiquetées');
  };
}

function clearSelection(redraw = true) {
  selection.clear();
  document.getElementById('selection-bar')?.remove();
  if (redraw) renderLists();
}

function showListMenu(anchor, list) {
  const existing = document.querySelector('.list-menu');
  existing?.remove();
  const menu = document.createElement('div');
  menu.className = 'list-menu';
  menu.style.width = '200px';
  const r = anchor.getBoundingClientRect();
  menu.style.top = `${r.bottom + 6}px`;
  menu.style.left = `${Math.min(window.innerWidth - 215, Math.max(8, r.left - 160))}px`;
  menu.innerHTML = `
    <button data-act="left">${icon('chevronLeft', { size: 14 })} <span>Déplacer à gauche</span></button>
    <button data-act="right">${icon('chevronRight', { size: 14 })} <span>Déplacer à droite</span></button>
    <button data-act="duplicate">${icon('copy', { size: 14 })} <span>Duplier la liste</span></button>
    <button data-act="wip">${icon('settings', { size: 14 })} <span>Limite WIP…</span></button>
    <button data-act="archive">${icon('boards', { size: 14 })} <span>${list.archived ? 'Désarchiver' : 'Archiver la liste'}</span></button>
    <button class="danger" data-act="delete">${icon('trash', { size: 14 })} <span>Supprimer la liste</span></button>
  `;
  document.getElementById('overlays').appendChild(menu);
  const close = () => menu.remove();
  setTimeout(() => document.addEventListener('pointerdown', function h(e) {
    if (!menu.contains(e.target) && e.target !== anchor) { close(); document.removeEventListener('pointerdown', h); }
  }), 0);
  menu.addEventListener('click', async (e) => {
    const act = e.target.closest('[data-act]')?.dataset.act;
    if (!act) return;
    close();
    if (act === 'left' || act === 'right') {
      try {
        await api.moveList(list.id, act === 'left' ? -1 : 1);
        reload();
      } catch (e) { toast(e.message, 'err'); }
    } else if (act === 'duplicate') {
      try {
        const nl = await api.duplicateList(list.id);
        toast(`Liste dupliée : « ${nl.title} »`, 'ok');
        reload();
      } catch (e) { toast(e.message, 'err'); }
    } else if (act === 'wip') {
      const v = await promptDialog('Limite WIP (vide = aucune)', list.wip_limit ? String(list.wip_limit) : '');
      await api.updateList(list.id, { wip_limit: v ? Number(v) : null });
      reload();
    } else if (act === 'archive') {
      await api.updateList(list.id, { archived: !list.archived });
      reload();
    } else if (act === 'delete') {
      const { confirmDialog } = await import('../ui/modal.js');
      if (await confirmDialog(`Supprimer « ${list.title} » et ses cartes ?`)) {
        await api.deleteList(list.id);
        reload();
      }
    }
  });
}

async function quickAdd(listId, body) {
  const existing = body.querySelector('.quick-add-form');
  if (existing) {
    existing.querySelector('textarea')?.focus();
    return;
  }

  const form = document.createElement('div');
  form.className = 'quick-add-form card-tile fade-in';
  form.style.padding = '8px 10px';
  form.innerHTML = `
    <textarea class="input" placeholder="Titre de la carte…" rows="2" style="min-height:54px;padding:6px 8px;font-size:13px;resize:none;margin-bottom:8px"></textarea>
    <div class="row" style="gap:6px">
      <button class="btn primary sm" data-submit style="flex:none">Ajouter</button>
      <button class="btn ghost sm" data-cancel style="flex:none">Annuler</button>
      <span class="muted" style="font-size:11px;margin-left:auto"><kbd class="kbd">Entrée ↵</kbd></span>
    </div>
  `;
  body.appendChild(form);
  const textarea = form.querySelector('textarea');
  textarea.focus();

  const cancel = () => form.remove();
  const submit = async () => {
    const title = textarea.value.trim();
    if (!title) { cancel(); return; }
    form.remove();
    try {
      await api.createCard(state.board.id, { title, list_id: listId });
      await reload();
    } catch (e) { toast(e.message, 'err'); }
  };

  form.querySelector('[data-submit]').onclick = submit;
  form.querySelector('[data-cancel]').onclick = cancel;
  textarea.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); submit(); }
    if (e.key === 'Escape') { e.preventDefault(); cancel(); }
  });
}

/** Galerie des modèles du board : instancie une carte dans la liste donnée. */
function openTemplateGallery(listId) {
  const templates = state.cards
    .filter((c) => c.is_template && !c.archived)
    .sort((a, b) => a.position - b.position);
  const rows = templates.map((t) => {
    const desc = t.template_description || t.description || '';
    const counts = [
      t._checklists_count ? `${t._checklists_count} checklist${t._checklists_count > 1 ? 's' : ''}` : '',
      t._label_ids?.length ? `${t._label_ids.length} label${t._label_ids.length > 1 ? 's' : ''}` : '',
      t._attachments_count ? `${t._attachments_count} pièce${t._attachments_count > 1 ? 's' : ''} jointe${t._attachments_count > 1 ? 's' : ''}` : '',
    ].filter(Boolean).join(' · ');
    return `
      <button class="tpl-row" data-tpl="${t.id}">
        <b>${escapeHtml(t.title)}</b>
        ${desc ? `<span class="muted" style="font-size:12px">${escapeHtml(desc.length > 110 ? `${desc.slice(0, 110)}…` : desc)}</span>` : ''}
        ${counts ? `<span class="mini muted">${counts}</span>` : ''}
      </button>`;
  }).join('');
  const m = openModal(`
    <div class="modal-head">
      <h2>Créer depuis un modèle</h2>
      <button class="iconbtn" data-close title="Fermer">${icon('close', { size: 16 })}</button>
    </div>
    <div class="modal-body" style="display:flex;flex-direction:column;gap:8px">
      ${rows || '<div class="muted" style="padding:8px 2px">Aucun modèle sur ce board. Ouvrez une carte puis « En faire un modèle ».</div>'}
    </div>`, { id: 'template-gallery' });
  m.el.querySelector('.modal-body').addEventListener('click', async (e) => {
    const id = e.target.closest('[data-tpl]')?.dataset.tpl;
    if (!id) return;
    const tpl = state.cards.find((c) => c.id === id);
    m.close();
    const title = await promptDialog('Titre de la nouvelle carte', tpl?.title || '');
    if (!title) return;
    try {
      const nc = await api.instantiateTemplate(id, { list_id: listId, name: title, at: 'bottom' });
      await reload();
      openCardModal(nc.id, { onChange: reload });
    } catch (err) { toast(err.message, 'err'); }
  });
}

function applyFocusVisual() {
  document.querySelectorAll('.card-tile.keyboard-focused').forEach((c) => c.classList.remove('keyboard-focused'));
  if (!focusedCardId) return;
  const tile = document.querySelector(`.card-tile[data-card="${focusedCardId}"]`);
  if (tile) {
    tile.classList.add('keyboard-focused');
    tile.scrollIntoView({ block: 'nearest', inline: 'nearest', behavior: 'smooth' });
  }
}

/** Navigation clavier avancée (flèches, J/K, H/L, Espace, E, A, ?) */
function initBoardKeyNav() {
  if (keyNavAttached) return;
  keyNavAttached = true;

  window.addEventListener('keydown', async (e) => {
    if (state.view !== 'board' || !state.board) return;
    const inField = ['INPUT', 'TEXTAREA', 'SELECT'].includes(document.activeElement?.tagName);
    if (inField) return;
    if (document.querySelector('.modal-backdrop, .palette-backdrop, .list-menu')) return;

    const lists = visibleLists();
    if (!lists.length) return;

    // Raccourci d'aide '?'
    if (e.key === '?' || (e.key === '/' && e.shiftKey)) {
      e.preventDefault();
      openShortcutsDialog();
      return;
    }

    // Si aucune carte n'est sélectionnée, initialiser sur la 1ère carte
    if (!focusedCardId) {
      for (const l of lists) {
        const cs = cardsOfList(l.id);
        if (cs.length) { focusedCardId = cs[0].id; applyFocusVisual(); break; }
      }
      return;
    }

    const currentCard = state.cards.find((c) => c.id === focusedCardId);
    if (!currentCard) return;

    const currentListIndex = lists.findIndex((l) => l.id === currentCard.list_id);
    const currentListCards = cardsOfList(currentCard.list_id);
    const currentCardIndex = currentListCards.findIndex((c) => c.id === currentCard.id);

    const key = e.key.toLowerCase();

    // Haut (K ou ArrowUp)
    if (e.key === 'ArrowUp' || key === 'k') {
      e.preventDefault();
      if (currentCardIndex > 0) {
        focusedCardId = currentListCards[currentCardIndex - 1].id;
        applyFocusVisual();
      }
    }
    // Bas (J ou ArrowDown)
    else if (e.key === 'ArrowDown' || key === 'j') {
      e.preventDefault();
      if (currentCardIndex < currentListCards.length - 1) {
        focusedCardId = currentListCards[currentCardIndex + 1].id;
        applyFocusVisual();
      }
    }
    // Gauche (H ou ArrowLeft)
    else if (e.key === 'ArrowLeft' || key === 'h') {
      e.preventDefault();
      if (currentListIndex > 0) {
        const prevListCards = cardsOfList(lists[currentListIndex - 1].id);
        if (prevListCards.length) {
          const targetIdx = Math.min(currentCardIndex, prevListCards.length - 1);
          focusedCardId = prevListCards[targetIdx].id;
          applyFocusVisual();
        }
      }
    }
    // Droite (L ou ArrowRight)
    else if (e.key === 'ArrowRight' || key === 'l') {
      e.preventDefault();
      if (currentListIndex < lists.length - 1) {
        const nextListCards = cardsOfList(lists[currentListIndex + 1].id);
        if (nextListCards.length) {
          const targetIdx = Math.min(currentCardIndex, nextListCards.length - 1);
          focusedCardId = nextListCards[targetIdx].id;
          applyFocusVisual();
        }
      }
    }
    // Entrée : ouvrir la carte
    else if (e.key === 'Enter') {
      e.preventDefault();
      openCardModal(focusedCardId, { onChange: reload });
    }
    // Espace : basculer terminée
    else if (e.key === ' ') {
      e.preventDefault();
      const nv = !currentCard.due_complete;
      currentCard.due_complete = nv;
      updateCardTileInDOM(currentCard.id);
      try {
        await api.updateCard(currentCard.id, { due_complete: nv });
        toast(nv ? '✓ Tâche terminée !' : 'Tâche marquée à faire', 'ok');
      } catch (err) {
        currentCard.due_complete = !nv;
        updateCardTileInDOM(currentCard.id);
        toast(err.message, 'err');
      }
    }
    // 'e' : édition rapide du titre inline
    else if (key === 'e' && !e.metaKey && !e.ctrlKey) {
      e.preventDefault();
      quickEditTileTitle(focusedCardId);
    }
    // 'a' : ajouter une carte dans la liste active
    else if (key === 'a' && !e.metaKey && !e.ctrlKey) {
      e.preventDefault();
      const listElNode = document.querySelector(`.list[data-list="${currentCard.list_id}"]`);
      if (listElNode) {
        quickAdd(currentCard.list_id, listElNode.querySelector('.list-body'));
      }
    }
    // '<' / '>' : déplacer la carte focusée vers la liste précédente/suivante
    // — alternative clavier au drag & drop (accessibilité), ajout en fin de liste
    else if ((e.key === '<' || e.key === '>') && !e.metaKey && !e.ctrlKey) {
      e.preventDefault();
      const dir = e.key === '<' ? -1 : 1;
      const target = lists[currentListIndex + dir];
      if (!target) return;
      try {
        await window.__krelloMoveCard(focusedCardId, { list_id: target.id });
        await reload();
        applyFocusVisual();
        toast(`Carte déplacée vers « ${target.title} »`, 'ok');
      } catch (err) {
        toast(err.message, 'err');
      }
    }
  });
}

function quickEditTileTitle(cardId) {
  const tile = document.querySelector(`.card-tile[data-card="${cardId}"]`);
  if (!tile) return;
  const titleEl = tile.querySelector('.title');
  if (!titleEl) return;
  const currentTitle = titleEl.textContent.trim();

  const input = document.createElement('input');
  input.className = 'input';
  input.value = currentTitle;
  input.style.cssText = 'padding:3px 6px;font-size:13.5px;font-weight:550;width:100%';
  titleEl.replaceWith(input);
  input.focus();
  input.select();

  let done = false;
  const finish = async (save) => {
    if (done) return;
    done = true;
    const nv = input.value.trim();
    if (save && nv && nv !== currentTitle) {
      const card = state.cards.find((c) => c.id === cardId);
      if (card) card.title = nv;
      input.replaceWith(titleEl);
      titleEl.textContent = nv;
      try {
        await api.updateCard(cardId, { title: nv });
        updateCardTileInDOM(cardId);
      } catch (err) {
        toast(err.message, 'err');
        await reload();
      }
    } else {
      input.replaceWith(titleEl);
    }
  };

  input.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') finish(true);
    if (e.key === 'Escape') finish(false);
  });
  input.addEventListener('blur', () => finish(true));
}

export function openShortcutsDialog() {
  openModal(`
    <div class="dialog-header">
      <div class="dialog-badge">${icon('boards', { size: 18 })}</div>
      <div class="dialog-titles">
        <h3 class="dialog-title">Raccourcis Clavier Krello</h3>
        <p class="dialog-desc">Naviguez et gérez vos projets et tableaux à la vitesse de la pensée.</p>
      </div>
      <button class="iconbtn dialog-close" data-close title="Fermer (Échap)">${icon('close', { size: 14 })}</button>
    </div>
    <div class="dialog-body" style="font-size:13px">
      <div style="display:grid;grid-template-columns:1fr 1fr;gap:22px">
        <div>
          <div class="modal-section" style="margin-bottom:0">
            <div class="label">Navigation Kanban</div>
            <div style="display:flex;flex-direction:column;gap:9px">
              <div style="display:flex;align-items:center;justify-content:space-between"><span>Naviguer entre cartes</span> <div><span class="kbd">← ↓ ↑ →</span> <span class="muted" style="font-size:11px">ou</span> <span class="kbd">HJKL</span></div></div>
              <div style="display:flex;align-items:center;justify-content:space-between"><span>Ouvrir la carte</span> <span class="kbd">Entrée ↵</span></div>
              <div style="display:flex;align-items:center;justify-content:space-between"><span>Terminer / réouvrir</span> <span class="kbd">Espace</span></div>
              <div style="display:flex;align-items:center;justify-content:space-between"><span>Édition rapide du titre</span> <span class="kbd">E</span></div>
              <div style="display:flex;align-items:center;justify-content:space-between"><span>Nouvelle carte</span> <span class="kbd">A</span></div>
              <div style="display:flex;align-items:center;justify-content:space-between"><span>Déplacer la carte focusée de liste</span> <div><span class="kbd">&lt;</span> <span class="kbd">&gt;</span></div></div>
              <div style="display:flex;align-items:center;justify-content:space-between"><span>Sélection multiple (actions en masse)</span> <div><span class="kbd">⌘clic</span> <span class="muted" style="font-size:11px">ou</span> <span class="kbd">⇧clic</span></div></div>
            </div>
          </div>
        </div>
        <div>
          <div class="modal-section" style="margin-bottom:0">
            <div class="label">Général & Système</div>
            <div style="display:flex;flex-direction:column;gap:9px">
              <div style="display:flex;align-items:center;justify-content:space-between"><span>Palette commandes</span> <span class="kbd">⌘K</span></div>
              <div style="display:flex;align-items:center;justify-content:space-between"><span>Annuler</span> <span class="kbd">⌘Z</span></div>
              <div style="display:flex;align-items:center;justify-content:space-between"><span>Rétablir</span> <span class="kbd">⇧⌘Z</span></div>
              <div style="display:flex;align-items:center;justify-content:space-between"><span>Carte IA naturelle</span> <span class="kbd">N</span></div>
              <div style="display:flex;align-items:center;justify-content:space-between"><span>Fermer le dialogue</span> <span class="kbd">Échap</span></div>
              <div style="display:flex;align-items:center;justify-content:space-between"><span>Raccourcis clavier</span> <span class="kbd">?</span></div>
            </div>
          </div>
        </div>
      </div>
    </div>
    <div class="dialog-footer">
      <div class="dialog-hints">
        <span class="kbd">Échap</span> pour fermer
      </div>
      <div class="dialog-btns">
        <button class="btn primary" data-close>Fermer</button>
      </div>
    </div>`, { size: 'md' });
}

async function reload({ refetch = true } = {}) {
  if (refetch) await refreshBoard();
  const scroller = host?.querySelector('.board-scroller');
  const scrollLeft = scroller?.scrollLeft ?? 0;
  const scrollTop = scroller?.scrollTop ?? 0;

  if (host && host.querySelector('#board')) {
    updateFilterBar();
    renderLists();
    if (scroller) {
      scroller.scrollLeft = scrollLeft;
      scroller.scrollTop = scrollTop;
    }
  } else {
    renderBoard();
  }
}
window.__krelloReloadBoard = reload;

/** Déplacement de carte utilisé par le DnD (exposé au module card_tile). */
window.__krelloMoveCard = async (cardId, move) => {
  await api.moveCard(cardId, move);
  await refreshBoard();
};

export function isDragging() {
  return activeDrag() !== null;
}

export { openCardModal };
