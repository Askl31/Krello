// Palette de commandes ⌘K : navigation, recherche globale, actions.

import { api } from '../api.js';
import { escapeHtml } from '../format.js';
import { state, loadBoards } from '../state.js';
import { boardUrl, navigate } from '../router.js';
import { toast } from './modal.js';

let paletteEl = null;

function openPalette() {
  if (paletteEl) return;
  const backdrop = document.createElement('div');
  backdrop.className = 'palette-backdrop';
  backdrop.innerHTML = `
    <div class="palette">
      <input placeholder="Rechercher un board, une carte, lancer une action…" />
      <div class="results"></div>
    </div>`;
  document.getElementById('overlays').appendChild(backdrop);
  paletteEl = backdrop;

  const input = backdrop.querySelector('input');
  const results = backdrop.querySelector('.results');
  let items = [];
  let selected = 0;

  const close = () => { backdrop.remove(); paletteEl = null; };
  backdrop.addEventListener('pointerdown', (e) => { if (e.target === backdrop) close(); });

  function render() {
    if (!items.length) {
      results.innerHTML = `<div class="empty">Aucun résultat</div>`;
      return;
    }
    results.innerHTML = items.map((it, i) => `
      <button class="res ${i === selected ? 'selected' : ''}" data-i="${i}">
        <span class="icon">${it.icon}</span>
        <span>${escapeHtml(it.title)}</span>
        ${it.sub ? `<span class="sub">${escapeHtml(it.sub)}</span>` : ''}
      </button>`).join('');
    results.querySelectorAll('.res').forEach((b) => {
      b.onclick = () => run(items[Number(b.dataset.i)]);
    });
    results.querySelector('.res.selected')?.scrollIntoView({ block: 'nearest' });
  }

  async function run(it) {
    close();
    try {
      await it.run();
    } catch (e) {
      toast(e.message, 'err');
    }
  }

  function baseItems() {
    const items = [];
    for (const b of state.boards) {
      items.push({
        icon: '🗂', title: b.title, sub: 'board',
        run: () => navigate(boardUrl(b.id, 'board')),
      });
    }
    if (state.board && state.aiStatus?.available) {
      items.push(
        { icon: '✨', title: 'Carte en langage naturel', sub: 'N',
          run: async () => { const m = await import('../components/ai_panel.js'); m.openNaturalCardDialog(); } },
      );
    }
    items.push(
      { icon: '＋', title: 'Nouveau board…', run: async () => { const { promptDialog } = await import('./modal.js'); const name = await promptDialog('Nom du nouveau board', '', 'Ex. : Projet Sirius'); if (name) { const b = await api.createBoard({ title: name }); await loadBoards(); navigate(boardUrl(b.id, 'board')); } } },
      { icon: '↩', title: 'Annuler (undo)', run: async () => { const r = await api.undo(); toast(r?.undone ? `Annulé : ${r.undone}` : 'Rien à annuler', r?.undone ? 'ok' : ''); } },
      { icon: '↪', title: 'Rétablir (redo)', run: async () => { const r = await api.redo(); toast(r?.redone ? `Rétabli : ${r.redone}` : 'Rien à rétablir', r?.redone ? 'ok' : ''); } },
      { icon: '⚙', title: 'Paramètres', run: () => navigate('#/settings') },
    );
    for (const v of [['board', 'Vue Kanban'], ['calendar', 'Vue Calendrier'], ['timeline', 'Vue Timeline (Gantt)'], ['table', 'Vue Table'], ['dashboard', 'Vue Dashboard']]) {
      if (state.board) {
        items.push({ icon: '👁', title: v[1], sub: 'board courant', run: () => navigate(boardUrl(state.board.id, v[0])) });
      }
    }
    return items;
  }

  async function search(q) {
    const qLower = q.trim().toLowerCase();
    if (!qLower) return baseItems();
    const base = baseItems().filter((it) => it.title.toLowerCase().includes(qLower));
    const hits = await api.search(q, state.board?.id);
    const cardItems = hits.slice(0, 12).map((c) => ({
      icon: '📄',
      title: c.title,
      sub: state.board && c.board_id === state.board.id ? 'carte' : 'autre board',
      run: async () => {
        const { openCardModal } = await import('../components/card_modal.js');
        await openCardModal(c.id);
      },
    }));
    return [...cardItems, ...base];
  }

  function update() {
    search(input.value).then((its) => { items = its; selected = 0; render(); }).catch(() => { items = []; render(); });
  }

  input.addEventListener('input', update);
  input.addEventListener('keydown', (e) => {
    if (e.key === 'ArrowDown') { selected = Math.min(selected + 1, items.length - 1); render(); e.preventDefault(); }
    else if (e.key === 'ArrowUp') { selected = Math.max(selected - 1, 0); render(); e.preventDefault(); }
    else if (e.key === 'Enter') { items[selected] && run(items[selected]); }
  });

  update();
  input.focus();
}

export function initPalette() {
  window.addEventListener('keydown', (e) => {
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'k') {
      e.preventDefault();
      openPalette();
    }
  });
}
