// Barre supérieure : logo, titre du board, onglets de vue, recherche, thème, IA.

import { api } from '../api.js';
import { escapeHtml } from '../format.js';
import { state, applyTheme, getResolvedTheme, loadBoards, loadBoard } from '../state.js';
import { boardUrl, navigate } from '../router.js';
import { on } from '../ws.js';
import { toast, promptDialog } from '../ui/modal.js';
import { initAiDrawer } from './ai_panel.js';
import { icon } from '../icons.js';

const VIEWS = [
  ['board', 'Kanban', 'kanban'],
  ['calendar', 'Calendrier', 'calendar'],
  ['timeline', 'Timeline', 'timeline'],
  ['table', 'Table', 'table'],
  ['dashboard', 'Dashboard', 'dashboard'],
];

export function renderTopbar(host) {
  const route = location.hash;
  const inBoard = state.board && route.includes(state.board.id);
  host.innerHTML = `
    <button class="iconbtn" id="tb-boards" title="Afficher/masquer les boards">${icon('sidebar', { size: 16 })}</button>
    <div class="logo">K</div>
    <h1 ${inBoard ? 'id="tb-title" title="Cliquez pour renommer" style="cursor:pointer"' : ''}>${inBoard ? escapeHtml(state.board.title) : 'Krello'}</h1>
    <div class="spacer"></div>
    ${inBoard ? `
      <div class="viewtabs" id="tb-views">
        ${VIEWS.map(([v, label, ic]) => `
          <button data-view="${v}" class="${state.view === v ? 'active' : ''}">
            ${icon(ic, { size: 14 })}
            <span>${label}</span>
          </button>`).join('')}
      </div>
      ${state.aiStatus?.available ? `
        <button class="iconbtn" id="tb-ai" title="Assistant IA Apple Intelligence (✨ / Raccourci N)">
          ${icon('ai', { size: 16 })}
        </button>` : ''}
    ` : ''}
    <button class="iconbtn" id="tb-undo" title="Annuler (⌘Z)">${icon('undo', { size: 15 })}</button>
    <button class="iconbtn" id="tb-redo" title="Rétablir (⇧⌘Z)">${icon('redo', { size: 15 })}</button>
    <button class="iconbtn" id="tb-theme" title="Basculer thème (${getResolvedTheme() === 'dark' ? 'passer au mode clair' : 'passer au mode sombre'})">
      ${icon(getResolvedTheme() === 'dark' ? 'sun' : 'moon', { size: 15 })}
    </button>
  `;

  host.querySelector('#tb-boards')?.addEventListener('click', () => {
    const sb = document.querySelector('.sidebar');
    if (sb) {
      sb.classList.toggle('hidden');
    }
  });

  host.querySelector('#tb-title')?.addEventListener('click', async () => {
    if (!state.board) return;
    const newTitle = await promptDialog('Renommer le board', state.board.title);
    if (newTitle && newTitle !== state.board.title) {
      await api.updateBoard(state.board.id, { title: newTitle });
      await loadBoard(state.board.id);
      await loadBoards();
      renderTopbar(host);
    }
  });

  host.querySelector('#tb-views')?.addEventListener('click', (e) => {
    const v = e.target.closest('[data-view]')?.dataset.view;
    if (v) navigate(boardUrl(state.board.id, v));
  });

  host.querySelector('#tb-ai')?.addEventListener('click', () => initAiDrawer());
  host.querySelector('#tb-undo')?.addEventListener('click', doUndo);
  host.querySelector('#tb-redo')?.addEventListener('click', doRedo);
  host.querySelector('#tb-theme')?.addEventListener('click', () => {
    const current = getResolvedTheme();
    state.theme = current === 'dark' ? 'light' : 'dark';
    applyTheme();
    renderTopbar(host);
    const route = location.hash;
    if (route.includes('/timeline')) {
      import('../views/timeline.js').then((m) => m.renderTimeline());
    } else if (route.includes('/dashboard')) {
      import('../views/dashboard.js').then((m) => m.renderDashboard());
    }
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
    } else if (e.key.toLowerCase() === 'n' && !e.metaKey && !e.ctrlKey && !e.altKey) {
      // N = carte en langage naturel (IA) — board ouvert requis, IA masquée sinon.
      if (!state.board || !state.aiStatus?.available) return;
      if (document.querySelector('.modal-backdrop, .palette-backdrop')) return;
      e.preventDefault();
      import('../components/ai_panel.js').then((m) => m.openNaturalCardDialog());
    }
  });
  on('UndoApplied', () => { /* les handlers d'écran réagissent via board events */ });
}
