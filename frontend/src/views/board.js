// Vue Kanban : listes + cartes, drag & drop, navigation clavier power-user, quick add, WIP.

import { api } from '../api.js';
import { escapeHtml, PRIORITY_META } from '../format.js';
import { state, cardsOfList, visibleLists, refreshBoard } from '../state.js';
import { draggable, activeDrag } from '../dnd.js';
import { toast, promptDialog, openModal } from '../ui/modal.js';
import { cardTileHtml, attachCardTile } from '../components/card_tile.js';
import { openCardModal } from '../components/card_modal.js';
import { icon } from '../icons.js';

let host = null;
let focusedCardId = null;
let keyNavAttached = false;

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
  const total = state.cards.filter((c) => !c.archived).length;
  const hasFilters = f.text || f.labelId || f.priority || f.due || f.includeArchived;
  bar.innerHTML = `
    <span class="flabel">${icon('filter', { size: 13 })} <span>Filtres</span></span>
    <input class="input" id="f-text" placeholder="Rechercher par titre ou description…" value="${escapeHtml(f.text)}" />
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
    <label class="row" style="gap:6px;flex:none;cursor:pointer">
      <input type="checkbox" id="f-archived" ${f.includeArchived ? 'checked' : ''} style="width:14px;height:14px" />
      <span class="flabel">Archivés</span>
    </label>
    ${hasFilters ? `<button class="iconbtn" id="f-reset" title="Réinitialiser les filtres" style="width:26px;height:26px">${icon('close', { size: 12 })}</button>` : ''}
    <span class="fcount">${visible}/${total} cartes</span>
    <button class="iconbtn" id="f-help" title="Raccourcis clavier (?)" style="width:26px;height:26px;margin-left:4px">
      <span class="kbd" style="font-size:11px">?</span>
    </button>`;

  bar.querySelector('#f-text').addEventListener('input', (e) => {
    state.filters.text = e.target.value;
    const pos = e.target.selectionStart;
    renderLists();
    const again = host.querySelector('#f-text');
    again?.focus();
    again?.setSelectionRange(pos, pos);
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
}

function renderLists() {
  const board = host.querySelector('#board');
  board.innerHTML = '';

  for (const list of visibleLists()) {
    board.appendChild(listEl(list));
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
  applyFocusVisual();
}

/** Met à jour compteur + bouton reset sans reconstruire la barre (focus conservé). */
function updateFilterBar() {
  const bar = host.querySelector('#board-filters');
  if (!bar) return;
  const f = state.filters;
  const visible = visibleLists().reduce((s, l) => s + cardsOfList(l.id).length, 0);
  const total = state.cards.filter((c) => !c.archived).length;
  const count = bar.querySelector('.fcount');
  if (count) count.textContent = `${visible}/${total} cartes`;
  const hasFilters = !!(f.text || f.labelId || f.priority || f.due || f.includeArchived);
  let reset = bar.querySelector('#f-reset');
  if (hasFilters && !reset) {
    reset = document.createElement('button');
    reset.className = 'iconbtn';
    reset.id = 'f-reset';
    reset.title = 'Réinitialiser les filtres';
    reset.style.cssText = 'width:26px;height:26px';
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
      ${cards.map(cardTileHtml).join('')}
    </div>
    <button class="add-card" data-add-card="${list.id}">
      ${icon('plus', { size: 13 })}
      <span>Ajouter une carte</span>
    </button>
  `;

  // interactions cartes
  el.querySelectorAll('.card-tile').forEach((tile) => {
    const card = state.cards.find((c) => c.id === tile.dataset.card);
    attachCardTile(tile, card, { onChange: reload });
    tile.addEventListener('click', () => {
      focusedCardId = card.id;
      applyFocusVisual();
    });
  });

  // quick add
  el.querySelector('[data-add-card]').onclick = () => quickAdd(list.id, el.querySelector('.list-body'));

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
      await api.updateCard(currentCard.id, { due_complete: nv });
      toast(nv ? '✓ Tâche terminée !' : 'Tâche marquée à faire', 'ok');
      await reload();
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
      await api.updateCard(cardId, { title: nv });
      await reload();
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

async function reload() {
  await refreshBoard();
  renderBoard();
}

/** Déplacement de carte utilisé par le DnD (exposé au module card_tile). */
window.__krelloMoveCard = async (cardId, move) => {
  await api.moveCard(cardId, move);
  await refreshBoard();
};

export function isDragging() {
  return activeDrag() !== null;
}

export { openCardModal };
