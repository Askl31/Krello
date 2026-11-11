// Barre latérale : boards, actions de données (import/export, duplication).

import { api } from '../api.js';
import { escapeHtml } from '../format.js';
import { state, loadBoards } from '../state.js';
import { boardUrl, navigate } from '../router.js';
import { on } from '../ws.js';
import { toast, promptDialog, openModal } from '../ui/modal.js';
import { icon } from '../icons.js';

export function renderSidebar(host) {
  host.innerHTML = `
    <div class="brand">
      <div class="sidebar-avatar" title="Espace de travail Krello">
        <span class="avatar-glyph">👤</span>
      </div>
      <div class="logo">K</div>
      <b>Krello</b>
    </div>
    <div class="section">
      <div class="label">
        <span>Boards</span>
        <button class="iconbtn" id="sb-new" title="Nouveau board" style="width:24px;height:24px">
          ${icon('plus', { size: 14 })}
        </button>
      </div>
      <div id="sb-boards"></div>
    </div>
    <div class="section">
      <div class="label">Données & Outils</div>
      <button class="item" id="sb-import">
        ${icon('import', { size: 15 })}
        <span>Importer…</span>
      </button>
      <button class="item" id="sb-settings">
        ${icon('settings', { size: 15 })}
        <span>Paramètres</span>
      </button>
    </div>
    <div class="foot">
      <button class="search-trigger" id="sb-palette" title="Ouvrir la palette de commandes (⌘K)">
        <span style="display:flex;align-items:center;gap:7px">
          ${icon('search', { size: 14 })}
          <span>Rechercher…</span>
        </span>
        <span class="kbd">⌘K</span>
      </button>
    </div>
  `;

  const boardList = host.querySelector('#sb-boards');
  boardList.innerHTML = state.boards.map((b) => {
    const active = state.board?.id === b.id;
    const count = b.id === state.board?.id ? state.cards.filter((c) => !c.archived).length : null;
    const color = /^#/.test(b.background?.match(/"value":"(#[0-9a-fA-F]+)"/)?.[1] || '')
      ? b.background.match(/"value":"(#[0-9a-fA-F]+)"/)[1] : 'var(--accent)';
    return `
      <button class="item ${active ? 'active' : ''}" data-board="${b.id}">
        <span class="dot" style="background:${color};color:${color}"></span>
        <span class="name">${escapeHtml(b.title)}</span>
        ${count !== null ? `<span class="count">${count}</span>` : ''}
      </button>`;
  }).join('') || `<div class="muted" style="padding:8px 12px;font-size:12.5px">Aucun board</div>`;

  boardList.querySelectorAll('[data-board]').forEach((b) => {
    b.onclick = () => navigate(boardUrl(b.dataset.board, 'board'));
  });

  host.querySelector('#sb-new').onclick = async () => {
    const name = await promptDialog('Nom du nouveau board', '', 'Ex. : Lancement produit');
    if (!name) return;
    const b = await api.createBoard({ title: name });
    await loadBoards();
    navigate(boardUrl(b.id, 'board'));
  };

  host.querySelector('#sb-palette').onclick = () => {
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'k', metaKey: true }));
  };

  host.querySelector('#sb-settings').onclick = () => { location.hash = '#/settings'; };

  host.querySelector('#sb-import').onclick = () => showImportDialog();
}

function showImportDialog() {
  const { el } = openModal(`
    <div class="dialog-header">
      <div class="dialog-badge">${icon('import', { size: 18 })}</div>
      <div class="dialog-titles">
        <h3 class="dialog-title">Importer un board ou un projet</h3>
        <p class="dialog-desc">Collez le contenu d'un export JSON au format Krello ou Trello pour recréer vos tableaux et cartes.</p>
      </div>
      <button class="iconbtn dialog-close" data-close title="Fermer (Échap)">${icon('close', { size: 14 })}</button>
    </div>
    <div class="dialog-body">
      <textarea class="input" id="imp-json" style="min-height:170px;font-family:var(--mono);font-size:12px;line-height:1.5" placeholder='{"name": "Mon projet", "lists": [ ... ]}'></textarea>
    </div>
    <div class="dialog-footer">
      <div class="dialog-hints">
        <span class="kbd">JSON</span> Krello ou Trello
      </div>
      <div class="dialog-btns">
        <button class="btn" data-close>Annuler</button>
        <button class="btn" id="imp-krello">Importer JSON Krello</button>
        <button class="btn primary" id="imp-trello">Importer export Trello</button>
      </div>
    </div>`, { size: 'md' });
  const doImport = async (fn) => {
    const text = el.querySelector('#imp-json').value;
    if (!text.trim()) return toast('Collez le JSON à importer', 'err');
    try {
      const board = await fn(text);
      await loadBoards();
      el.closest('.modal-backdrop').remove();
      toast(`Board « ${board.title} » importé`, 'ok');
      navigate(boardUrl(board.id, 'board'));
    } catch (e) {
      toast(`Import impossible : ${e.message}`, 'err');
    }
  };
  el.querySelector('#imp-krello').onclick = () => doImport(api.importKrello);
  el.querySelector('#imp-trello').onclick = () => doImport(api.importTrello);
}

export function initSidebar(host) {
  renderSidebar(host);
  on('boards.changed', () => renderSidebar(host));
  on('board.loaded', () => renderSidebar(host));
  on('CardCreated', () => renderSidebar(host));
  on('CardDeleted', () => renderSidebar(host));
}
