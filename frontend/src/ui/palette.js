// Palette de commandes ⌘K : navigation, recherche globale, actions.

import { api } from '../api.js';
import { escapeHtml } from '../format.js';
import { state, loadBoards } from '../state.js';
import { boardUrl, navigate } from '../router.js';
import { toast } from './modal.js';
import { icon } from '../icons.js';

let paletteEl = null;

function openPalette() {
  if (paletteEl) return;
  const backdrop = document.createElement('div');
  backdrop.className = 'palette-backdrop';
  backdrop.innerHTML = `
    <div class="palette">
      <div class="palette-input-wrap">
        ${icon('search', { size: 18, className: 'muted' })}
        <input placeholder="Rechercher un board, une carte, lancer une action…" autofocus />
        <span class="kbd">Échap</span>
      </div>
      <div class="results"></div>
      <div class="palette-footer">
        <span>Navigation : <kbd class="kbd">↑</kbd> <kbd class="kbd">↓</kbd></span>
        <span>Ouvrir : <kbd class="kbd">↵</kbd></span>
      </div>
    </div>`;
  document.getElementById('overlays').appendChild(backdrop);
  paletteEl = backdrop;

  const input = backdrop.querySelector('input');
  const results = backdrop.querySelector('.results');
  let items = [];
  let selected = 0;
  let searchTimer = null;
  let searchSeq = 0;

  const close = () => {
    backdrop.style.opacity = '0';
    backdrop.style.transition = 'opacity 0.15s ease';
    setTimeout(() => { backdrop.remove(); paletteEl = null; }, 150);
  };

  backdrop.addEventListener('pointerdown', (e) => { if (e.target === backdrop) close(); });

  function render() {
    if (!items.length) {
      results.innerHTML = `<div class="empty">${icon('search', { size: 28, className: 'muted' })}<p style="margin:8px 0 0">Aucun résultat trouvé</p></div>`;
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
    items.push({
      icon: icon('dashboard', { size: 15 }),
      title: "Vue d'ensemble",
      sub: 'accueil',
      run: () => navigate('#/'),
    });
    for (const b of state.boards) {
      items.push({
        icon: icon('boards', { size: 15 }),
        title: b.title,
        sub: 'board',
        run: () => navigate(boardUrl(b.id, 'board')),
      });
    }
    if (state.board && state.aiStatus?.available) {
      items.push(
        {
          icon: icon('ai', { size: 15 }),
          title: 'Carte en langage naturel (Apple Intelligence)',
          sub: 'Raccourci N',
          run: async () => { const m = await import('../components/ai_panel.js'); m.openNaturalCardDialog(); },
        },
      );
    }
    items.push(
      {
        icon: icon('plus', { size: 15 }),
        title: 'Nouveau board…',
        run: async () => {
          const { promptDialog } = await import('./modal.js');
          const name = await promptDialog('Nom du nouveau board', '', 'Ex. : Projet Sirius');
          if (name) {
            const b = await api.createBoard({ title: name });
            await loadBoards();
            navigate(boardUrl(b.id, 'board'));
          }
        },
      },
      {
        icon: icon('undo', { size: 15 }),
        title: 'Annuler (undo)',
        sub: '⌘Z',
        run: async () => {
          const r = await api.undo();
          toast(r?.undone ? `Annulé : ${r.undone}` : 'Rien à annuler', r?.undone ? 'ok' : '');
        },
      },
      {
        icon: icon('redo', { size: 15 }),
        title: 'Rétablir (redo)',
        sub: '⇧⌘Z',
        run: async () => {
          const r = await api.redo();
          toast(r?.redone ? `Rétabli : ${r.redone}` : 'Rien à rétablir', r?.redone ? 'ok' : '');
        },
      },
      {
        icon: icon('settings', { size: 15 }),
        title: 'Paramètres généraux',
        sub: 'Apparence, sauvegardes, calendriers…',
        run: () => navigate('#/settings'),
      },
    );

    if (state.board) {
      items.push({
        icon: icon('kanban', { size: 15 }),
        title: `Paramètres du tableau « ${state.board.title} »`,
        sub: 'Ambiance, planning CPM, labels, champs, règles…',
        run: async () => {
          const m = await import('../components/board_settings_modal.js');
          m.openBoardSettingsModal(state.board.id);
        },
      });
    }

    const viewDefs = [
      ['board', 'Vue Kanban', 'kanban'],
      ['calendar', 'Vue Calendrier', 'calendar'],
      ['timeline', 'Vue Timeline (Gantt CPM)', 'timeline'],
      ['table', 'Vue Table', 'table'],
      ['dashboard', 'Vue Dashboard', 'dashboard'],
    ];

    for (const v of viewDefs) {
      if (state.board) {
        items.push({
          icon: icon(v[2], { size: 15 }),
          title: v[1],
          sub: 'vue courante',
          run: () => navigate(boardUrl(state.board.id, v[0])),
        });
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
      icon: icon('kanban', { size: 15 }),
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
    // Debounce 150 ms + ignore les réponses arrivées après une frappe plus récente.
    clearTimeout(searchTimer);
    searchTimer = setTimeout(async () => {
      const my = ++searchSeq;
      try {
        const its = await search(input.value);
        if (my !== searchSeq) return;
        items = its; selected = 0; render();
      } catch {
        if (my === searchSeq) { items = []; render(); }
      }
    }, 150);
  }

  input.addEventListener('input', update);
  input.addEventListener('keydown', (e) => {
    if (e.key === 'ArrowDown') { selected = Math.min(selected + 1, items.length - 1); render(); e.preventDefault(); }
    else if (e.key === 'ArrowUp') { selected = Math.max(selected - 1, 0); render(); e.preventDefault(); }
    else if (e.key === 'Enter') { items[selected] && run(items[selected]); }
    else if (e.key === 'Escape') { close(); }
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
