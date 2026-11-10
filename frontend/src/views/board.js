// Vue Kanban : listes + cartes, drag & drop, quick add, WIP.

import { api } from '../api.js';
import { escapeHtml } from '../format.js';
import { state, cardsOfList, visibleLists, refreshBoard } from '../state.js';
import { draggable, activeDrag } from '../dnd.js';
import { toast, promptDialog } from '../ui/modal.js';
import { cardTileHtml, attachCardTile } from '../components/card_tile.js';
import { openCardModal } from '../components/card_modal.js';

let host = null;

export function renderBoard() {
  host = document.querySelector('.view');
  host.innerHTML = `
    <div class="board-scroller">
      <div class="board" id="board"></div>
    </div>`;
  const board = host.querySelector('#board');

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
