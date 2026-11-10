// Barre latérale : boards, actions de données (import/export, duplication).

import { api } from '../api.js';
import { escapeHtml } from '../format.js';
import { state, loadBoards } from '../state.js';
import { boardUrl, navigate } from '../router.js';
import { on } from '../ws.js';
import { toast, confirmDialog, promptDialog, openModal } from '../ui/modal.js';

export function renderSidebar(host) {
  host.innerHTML = `
    <div class="brand"><div class="logo">K</div><b>Krello</b></div>
    <div class="section">
      <div class="label">Boards <button class="iconbtn" id="sb-new" title="Nouveau board" style="width:22px;height:22px;font-size:14px">＋</button></div>
      <div id="sb-boards"></div>
    </div>
    <div class="section">
      <div class="label">Données</div>
      <button class="item" id="sb-import">📥 Importer…</button>
      <button class="item" id="sb-settings">⚙ Paramètres</button>
    </div>
    <div class="foot">
      <button class="item" id="sb-palette">⌘K Rechercher…</button>
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
        <span class="dot" style="background:${color}"></span>
        <span class="name">${escapeHtml(b.title)}</span>
        ${count !== null ? `<span class="count">${count}</span>` : ''}
      </button>`;
  }).join('') || `<div class="muted" style="padding:4px 10px;font-size:12px">Aucun board</div>`;

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
    <div class="modal-head"><h2>Importer un board</h2><button class="iconbtn" data-close>✕</button></div>
    <div class="modal-body">
      <p class="muted" style="margin-top:0">Collez le contenu d'un export JSON — format Krello ou export Trello (boards/*.json).</p>
      <textarea class="input" id="imp-json" style="min-height:180px;font-family:var(--mono);font-size:12px" placeholder='{"name": "…"'></textarea>
      <div class="row" style="justify-content:flex-end;margin-top:12px">
        <button class="btn" id="imp-krello">Importer (Krello)</button>
        <button class="btn primary" id="imp-trello">Importer (Trello)</button>
      </div>
    </div>`);
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
