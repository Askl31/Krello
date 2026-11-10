// Barre supérieure : logo, titre du board, onglets de vue, recherche, thème, IA.

import { api } from '../api.js';
import { escapeHtml } from '../format.js';
import { state, applyTheme, loadBoards } from '../state.js';
import { boardUrl, navigate } from '../router.js';
import { on } from '../ws.js';
import { toast, promptDialog } from '../ui/modal.js';
import { initAiDrawer } from './ai_panel.js';

const VIEWS = [
  ['board', 'Kanban'],
  ['calendar', 'Calendrier'],
  ['timeline', 'Timeline'],
  ['table', 'Table'],
  ['dashboard', 'Dashboard'],
];

export function renderTopbar(host) {
  const route = location.hash;
  const inBoard = state.board && route.includes(state.board.id);
  host.innerHTML = `
    <button class="iconbtn" id="tb-boards" title="Boards">☰</button>
    <div class="logo">K</div>
    <h1>${inBoard ? escapeHtml(state.board.title) : 'Krello'}</h1>
    <div class="spacer"></div>
    ${inBoard ? `
      <div class="viewtabs" id="tb-views">
        ${VIEWS.map(([v, label]) => `<button data-view="${v}" class="${state.view === v ? 'active' : ''}">${label}</button>`).join('')}
      </div>
      ${state.aiStatus?.available ? '<button class="iconbtn" id="tb-ai" title="Assistant IA (Apple Intelligence)">✨</button>' : ''}
    ` : ''}
    <button class="iconbtn" id="tb-undo" title="Annuler (⌘Z)">↩</button>
    <button class="iconbtn" id="tb-redo" title="Rétablir (⇧⌘Z)">↪</button>
    <button class="iconbtn" id="tb-theme" title="Thème clair/sombre">◐</button>
  `;

  host.querySelector('#tb-boards')?.addEventListener('click', () => {
    document.querySelector('.sidebar')?.classList.toggle('hidden');
    const sb = document.querySelector('.sidebar');
    if (sb) sb.style.display = sb.style.display === 'none' ? '' : 'none';
  });
  host.querySelector('#tb-views')?.addEventListener('click', (e) => {
    const v = e.target.closest('[data-view]')?.dataset.view;
    if (v) navigate(boardUrl(state.board.id, v));
  });
  host.querySelector('#tb-ai')?.addEventListener('click', () => initAiDrawer());
  host.querySelector('#tb-undo')?.addEventListener('click', doUndo);
  host.querySelector('#tb-redo')?.addEventListener('click', doRedo);
  host.querySelector('#tb-theme')?.addEventListener('click', () => {
    state.theme = state.theme === 'dark' ? 'light' : 'dark';
    applyTheme();
  });
}

export async function doUndo() {
  try {
    const r = await api.undo();
    toast(r?.undone ? `Annulé : ${r.undone}` : 'Rien à annuler', r?.undone ? 'ok' : '');
    if (r?.undone) { await loadBoards(); }
  } catch (e) { toast(e.message, 'err'); }
}

export async function doRedo() {
  try {
    const r = await api.redo();
    toast(r?.redone ? `Rétabli : ${r.redone}` : 'Rien à rétablir', r?.redone ? 'ok' : '');
  } catch (e) { toast(e.message, 'err'); }
}

export function initTopbarShortcuts() {
  window.addEventListener('keydown', (e) => {
    const inField = ['INPUT', 'TEXTAREA', 'SELECT'].includes(document.activeElement?.tagName);
    if (inField) return;
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'z' && !e.shiftKey) {
      e.preventDefault();
      doUndo();
    } else if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'z' && e.shiftKey) {
      e.preventDefault();
      doRedo();
    }
  });
  on('UndoApplied', () => { /* les handlers d'écran réagissent via board events */ });
}
