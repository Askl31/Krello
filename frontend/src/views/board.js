// Vue Kanban : listes + cartes, drag & drop, quick add, WIP.

import { api } from '../api.js';
import { escapeHtml, PRIORITY_META } from '../format.js';
import { state, cardsOfList, visibleLists, refreshBoard } from '../state.js';
import { draggable, activeDrag } from '../dnd.js';
import { toast, promptDialog } from '../ui/modal.js';
import { cardTileHtml, attachCardTile } from '../components/card_tile.js';
import { openCardModal } from '../components/card_modal.js';

let host = null;

export function renderBoard() {
  host = document.querySelector('.view');
  host.innerHTML = `
    <div class="filters" id="board-filters"></div>
    <div class="board-scroller">
      <div class="board" id="board"></div>
    </div>`;
  renderFilters();
  renderLists();
}

function renderFilters() {
  const bar = host.querySelector('#board-filters');
  const f = state.filters;
  const visible = visibleLists().reduce((s, l) => s + cardsOfList(l.id).length, 0);
  const total = state.cards.filter((c) => !c.archived).length;
  const hasFilters = f.text || f.labelId || f.priority || f.due || f.includeArchived;
  bar.innerHTML = `
    <span class="flabel">Filtres</span>
    <input class="input" id="f-text" placeholder="Titre ou description…" value="${escapeHtml(f.text)}" />
    <select class="input" id="f-label">
      <option value="">Tous les labels</option>
      ${state.labels.map((l) => `<option value="${l.id}" ${f.labelId === l.id ? 'selected' : ''}>${escapeHtml(l.name || '•')}</option>`).join('')}
    </select>
    <select class="input" id="f-priority">
      <option value="">Toutes priorités</option>
      ${Object.entries(PRIORITY_META).filter(([k]) => k !== 'none')
        .map(([k, v]) => `<option value="${k}" ${f.priority === k ? 'selected' : ''}>${v.label}</option>`).join('')}
    </select>
    <select class="input" id="f-due">
      <option value="">Échéances…</option>
      <option value="overdue" ${f.due === 'overdue' ? 'selected' : ''}>En retard</option>
      <option value="week" ${f.due === 'week' ? 'selected' : ''}>≤ 7 jours</option>
    </select>
    <label class="row" style="gap:5px;flex:none"><input type="checkbox" id="f-archived" ${f.includeArchived ? 'checked' : ''} style="width:14px;height:14px" /><span class="flabel">Archivés</span></label>
    ${hasFilters ? '<button class="iconbtn" id="f-reset" title="Réinitialiser les filtres" style="width:26px;height:26px;font-size:12px">✕</button>' : ''}
    <span class="fcount">${visible}/${total} cartes affichées</span>`;

  bar.querySelector('#f-text').addEventListener('input', (e) => {
    state.filters.text = e.target.value;
    const pos = e.target.selectionStart;
    renderLists();
    const again = host.querySelector('#f-text');
    again.focus();
    again.setSelectionRange(pos, pos);
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
}

function renderLists() {
  const board = host.querySelector('#board');
  board.innerHTML = '';

  for (const list of visibleLists()) {
    board.appendChild(listEl(list));
  }

  const addList = document.createElement('button');
  addList.className = 'list-add';
  addList.textContent = '＋ Ajouter une liste';
  addList.onclick = async () => {
    const title = await promptDialog('Nom de la liste');
    if (!title) return;
    try {
      await api.createList(state.board.id, { title });
      await reload();
    } catch (e) { toast(e.message, 'err'); }
  };
  board.appendChild(addList);
  updateFilterBar();
}

/** Met à jour compteur + bouton reset sans reconstruire la barre (focus conservé). */
function updateFilterBar() {
  const bar = host.querySelector('#board-filters');
  if (!bar) return;
  const f = state.filters;
  const visible = visibleLists().reduce((s, l) => s + cardsOfList(l.id).length, 0);
  const total = state.cards.filter((c) => !c.archived).length;
  const count = bar.querySelector('.fcount');
  if (count) count.textContent = `${visible}/${total} cartes affichées`;
  const hasFilters = !!(f.text || f.labelId || f.priority || f.due || f.includeArchived);
  let reset = bar.querySelector('#f-reset');
  if (hasFilters && !reset) {
    reset = document.createElement('button');
    reset.className = 'iconbtn';
    reset.id = 'f-reset';
    reset.title = 'Réinitialiser les filtres';
    reset.style.cssText = 'width:26px;height:26px;font-size:12px';
    reset.textContent = '✕';
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

  el.innerHTML = `
    <div class="list-head">
      <h3 data-edit-list="${list.id}">${escapeHtml(list.title)}</h3>
      <span class="count ${over ? 'wip-warn' : ''}">${cards.length}${list.wip_limit ? `/${list.wip_limit}` : ''}</span>
      <button class="iconbtn" data-list-menu="${list.id}" title="Menu liste" style="width:26px;height:26px;font-size:13px">⋯</button>
    </div>
    <div class="list-body" data-body="${list.id}">
      ${cards.map(cardTileHtml).join('')}
    </div>
    <button class="add-card" data-add-card="${list.id}">＋ carte</button>
  `;

  // interactions cartes
  el.querySelectorAll('.card-tile').forEach((tile) => {
    const card = state.cards.find((c) => c.id === tile.dataset.card);
    attachCardTile(tile, card, { onChange: reload });
  });

  // quick add
  el.querySelector('[data-add-card]').onclick = () => quickAdd(list.id, el.querySelector('.list-body'));

  // édition titre + menu
  el.querySelector('[data-edit-list]').addEventListener('click', async () => {
    const nv = await promptDialog('Renommer la liste', list.title);
    if (nv) { await api.updateList(list.id, { title: nv }); reload(); }
  });
  el.querySelector('[data-list-menu]').addEventListener('click', async (e) => {
    showListMenu(e.currentTarget, list);
  });

  // drag de la liste (poignée = en-tête)
  draggable(el, {
    kind: 'list',
    id: list.id,
    canDrag: (ev) => !!ev.target.closest('.list-head h3, .list-head'),
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

function showListMenu(anchor, list) {
  const existing = document.querySelector('.list-menu');
  existing?.remove();
  const menu = document.createElement('div');
  menu.className = 'list-menu ai-drawer';
  menu.style.width = '220px';
  const r = anchor.getBoundingClientRect();
  menu.style.top = `${r.bottom + 4}px`;
  menu.style.left = `${Math.max(8, r.left - 180)}px`;
  menu.innerHTML = `
    <div class="ai-row" style="flex-direction:column;align-items:stretch;gap:6px">
      <button class="btn sm" data-act="wip">Limite WIP…</button>
      <button class="btn sm" data-act="archive">${list.archived ? 'Désarchiver' : 'Archiver la liste'}</button>
      <button class="btn danger sm" data-act="delete">Supprimer la liste</button>
    </div>`;
  document.getElementById('overlays').appendChild(menu);
  const close = () => menu.remove();
  setTimeout(() => document.addEventListener('pointerdown', function h(e) {
    if (!menu.contains(e.target)) { close(); document.removeEventListener('pointerdown', h); }
  }), 0);
  menu.addEventListener('click', async (e) => {
    const act = e.target.closest('[data-act]')?.dataset.act;
    if (!act) return;
    close();
    if (act === 'wip') {
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
  const input = document.createElement('input');
  input.className = 'input';
  input.placeholder = 'Titre de la carte (Entrée pour valider)';
  body.appendChild(input);
  input.focus();
  let done = false;
  const submit = async () => {
    if (done) return;
    done = true;
    const title = input.value.trim();
    input.remove();
    if (!title) return;
    try {
      await api.createCard(state.board.id, { title, list_id: listId });
      await reload();
    } catch (e) { toast(e.message, 'err'); }
  };
  input.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') submit();
    if (e.key === 'Escape') { done = true; input.remove(); }
  });
  input.addEventListener('blur', () => submit());
}

async function reload() {
  await refreshBoard();
  renderBoard();
}

/** Déplacement de carte utilisé par le DnD (exposé au module card_tile). */
window.__krelloMoveCard = async (cardId, move) => {
  await api.moveCard(cardId, move);
  await hydrateLabelIds();
};

export function isDragging() {
  return activeDrag() !== null;
}

export { openCardModal };
