// Barre latérale : boards, actions de données (import/export, duplication).

import { api } from '../api.js';
import { escapeHtml } from '../format.js';
import { state, loadBoards } from '../state.js';
import { boardUrl, navigate } from '../router.js';
import { on } from '../ws.js';
import { toast, promptDialog, openModal, confirmDialog } from '../ui/modal.js';
import { icon } from '../icons.js';

export function renderSidebar(host) {
  const prevScrollTop = host.querySelector('.sidebar-body')?.scrollTop || 0;
  const isCollapsed = host.classList.contains('collapsed');
  const board = state.board;
  const boardsCollapsed = localStorage.getItem('krello-boards-collapsed') === 'true';

  host.innerHTML = `
    <div class="brand">
      <div class="sidebar-avatar" title="Espace de travail Krello">
        <span class="avatar-glyph">👤</span>
      </div>
      <div class="logo" title="Krello">${icon('logo', { size: 26 })}</div>
      <b>Krello</b>
      <button class="iconbtn sb-collapse-btn" id="sb-collapse-btn" title="Réduire la barre latérale">
        ${icon('sidebar', { size: 14 })}
      </button>
    </div>

    <div class="sidebar-body" id="sb-body">
      ${board ? `
      <div class="section rail-views">
        <div class="label">
          <span>Vues</span>
        </div>
        <button class="item ${state.view === 'board' ? 'active' : ''}" data-rail-view="board" title="Kanban">
          ${icon('kanban', { size: 15 })}
          <span class="name">Kanban</span>
        </button>
        <button class="item ${state.view === 'calendar' ? 'active' : ''}" data-rail-view="calendar" title="Calendrier">
          ${icon('calendar', { size: 15 })}
          <span class="name">Calendrier</span>
        </button>
        <button class="item ${state.view === 'timeline' ? 'active' : ''}" data-rail-view="timeline" title="Timeline">
          ${icon('timeline', { size: 15 })}
          <span class="name">Timeline</span>
        </button>
        <button class="item ${state.view === 'table' ? 'active' : ''}" data-rail-view="table" title="Table">
          ${icon('table', { size: 15 })}
          <span class="name">Table</span>
        </button>
        <button class="item ${state.view === 'dashboard' ? 'active' : ''}" data-rail-view="dashboard" title="Dashboard">
          ${icon('dashboard', { size: 15 })}
          <span class="name">Dashboard</span>
        </button>
      </div>` : ''}

      <div class="section sb-section-boards ${boardsCollapsed ? 'is-collapsed' : ''}">
        <div class="label">
          <button class="sb-section-toggle" id="sb-boards-toggle" type="button" title="${boardsCollapsed ? 'Développer les boards' : 'Réduire les boards'}" aria-expanded="${!boardsCollapsed}">
            <span class="sb-section-chevron">${icon('chevronDown', { size: 12 })}</span>
            <span>Boards</span>
            <span class="sb-boards-count-badge">${state.boards.length}</span>
          </button>
          <button class="iconbtn" id="sb-new" title="Nouveau board" style="width:24px;height:24px">
            ${icon('plus', { size: 14 })}
          </button>
        </div>
        <div id="sb-boards"></div>
      </div>

      <div class="section">
        <div class="label">
          <span>Données & Outils</span>
        </div>
        <button class="item" id="sb-import" title="Importer…">
          ${icon('import', { size: 15 })}
          <span class="name">Importer…</span>
        </button>
        <button class="item ${state.view === 'settings' ? 'active' : ''}" id="sb-settings" title="Paramètres">
          ${icon('settings', { size: 15 })}
          <span class="name">Paramètres</span>
        </button>
      </div>
    </div>

    <div class="foot">
      <button class="search-trigger" id="sb-palette" title="Ouvrir la palette de commandes (⌘K)">
        <span style="display:flex;align-items:center;gap:7px">
          ${icon('search', { size: 14 })}
          <span>Rechercher…</span>
        </span>
        <span class="kbd">⌘K</span>
      </button>
      <button class="item sb-expand-btn" id="sb-expand-btn" title="Agrandir la barre latérale">
        ${icon('sidebar', { size: 15 })}
        <span class="name">Agrandir</span>
      </button>
    </div>
  `;

  if (prevScrollTop > 0) {
    const sbBody = host.querySelector('.sidebar-body');
    if (sbBody) sbBody.scrollTop = prevScrollTop;
  }

  const boardList = host.querySelector('#sb-boards');
  boardList.innerHTML = state.boards.map((b) => {
    const active = state.board?.id === b.id;
    const count = b.id === state.board?.id ? state.cards.filter((c) => !c.archived && !c.is_template).length : null;
    let color = 'var(--accent)';
    let dotBg = 'var(--accent)';
    try {
      const c = JSON.parse(b.background || '{}');
      if (typeof c.value === 'string') {
        if (/^#/.test(c.value) && c.value !== '#101828') {
          color = c.value;
          dotBg = c.value;
        } else if (c.value.startsWith('linear-gradient') || c.value.startsWith('radial-gradient')) {
          dotBg = c.value;
          const m = c.value.match(/#(?:[0-9a-fA-F]{3}){1,2}/);
          if (m) color = m[0];
        }
      }
    } catch { /* JSON invalide → accent par défaut */ }
    return `
      <div class="sb-board-row" data-board-row="${b.id}">
        <button class="item ${active ? 'active' : ''}" data-board="${b.id}" title="${escapeHtml(b.title)}">
          <span class="dot" style="background:${dotBg};color:${color}"></span>
          <span class="name">${escapeHtml(b.title)}</span>
          ${count !== null ? `<span class="count">${count}</span>` : ''}
        </button>
        <div class="sb-board-actions">
          <button class="iconbtn sb-board-settings" data-settings-board="${b.id}" title="Paramètres du tableau « ${escapeHtml(b.title)} »">
            ${icon('settings', { size: 12 })}
          </button>
          <button class="iconbtn sb-board-del" data-del-board="${b.id}" title="Supprimer le tableau « ${escapeHtml(b.title)} »">
            ${icon('trash', { size: 12 })}
          </button>
        </div>
      </div>`;
  }).join('') || `<div class="muted sb-no-board" style="padding:8px 12px;font-size:12.5px">Aucun board</div>`;

  boardList.querySelectorAll('[data-board]').forEach((b) => {
    b.onclick = () => {
      navigate(boardUrl(b.dataset.board, 'board'));
    };
  });
  boardList.querySelectorAll('[data-settings-board]').forEach((b) => {
    b.onclick = (e) => {
      e.stopPropagation();
      import('./board_settings_modal.js').then((m) => m.openBoardSettingsModal(b.dataset.settingsBoard));
    };
  });
  boardList.querySelectorAll('[data-del-board]').forEach((b) => {
    b.onclick = async (e) => {
      e.stopPropagation();
      const bid = b.dataset.delBoard;
      const title = state.boards.find((x) => x.id === bid)?.title || bid;
      if (!(await confirmDialog(`Supprimer définitivement le board « ${title} » ? Toutes ses listes, cartes et règles seront supprimées (annulable avec ⌘Z dans la session).`))) return;
      try {
        await api.deleteBoard(bid);
        await loadBoards();
        toast(`Board « ${title} » supprimé`, 'ok');
        if (state.board?.id === bid) {
          if (state.boards.length) navigate(boardUrl(state.boards[0].id, 'board'));
          else { state.board = null; navigate('#/'); }
        } else {
          renderSidebar(host);
        }
      } catch (err) { toast(err.message, 'err'); }
    };
  });

  host.querySelectorAll('[data-rail-view]').forEach((b) => {
    b.onclick = () => {
      if (state.board) {
        navigate(boardUrl(state.board.id, b.dataset.railView));
      }
    };
  });

  const toggleCollapse = () => {
    host.classList.toggle('collapsed');
    const collapsed = host.classList.contains('collapsed');
    localStorage.setItem('krello-sidebar-collapsed', collapsed ? 'true' : 'false');
    api.saveUiSettings({ sidebar_collapsed: collapsed }).catch(() => {});
  };

  const toggleBoards = (e) => {
    e?.stopPropagation();
    const boardsSec = host.querySelector('.sb-section-boards');
    if (!boardsSec) return;
    boardsSec.classList.toggle('is-collapsed');
    const isNowCollapsed = boardsSec.classList.contains('is-collapsed');
    localStorage.setItem('krello-boards-collapsed', isNowCollapsed ? 'true' : 'false');
    const toggleBtn = host.querySelector('#sb-boards-toggle');
    if (toggleBtn) {
      toggleBtn.setAttribute('aria-expanded', (!isNowCollapsed).toString());
      toggleBtn.setAttribute('title', isNowCollapsed ? 'Développer les boards' : 'Réduire les boards');
    }
  };

  host.querySelector('#sb-collapse-btn')?.addEventListener('click', toggleCollapse);
  host.querySelector('#sb-expand-btn')?.addEventListener('click', toggleCollapse);
  host.querySelector('#sb-boards-toggle')?.addEventListener('click', toggleBoards);

  host.querySelector('#sb-new').onclick = async (e) => {
    e.stopPropagation();
    const name = await promptDialog('Nom du nouveau board', '', 'Ex. : Lancement produit');
    if (!name) return;
    const b = await api.createBoard({ title: name });
    localStorage.setItem('krello-boards-collapsed', 'false');
    await loadBoards();
    navigate(boardUrl(b.id, 'board'));
  };

  host.querySelector('#sb-palette').onclick = () => {
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'k', metaKey: true }));
  };

  host.querySelector('#sb-settings').onclick = () => {
    navigate('#/settings');
  };

  host.querySelector('#sb-import').onclick = () => showImportDialog();
}

function showImportDialog() {
  let selectedFormat = 'auto'; // 'auto' | 'krello' | 'trello'
  let activeMethod = 'file'; // 'file' | 'text'
  let rawText = '';
  let selectedFileName = '';
  let selectedFileSize = '';

  const { el } = openModal(`
    <div class="dialog-header">
      <div class="dialog-badge">${icon('import', { size: 18 })}</div>
      <div class="dialog-titles">
        <h3 class="dialog-title">Importer un board ou un projet</h3>
        <p class="dialog-desc">Importez vos tableaux et cartes depuis un export JSON Krello ou un export Trello.</p>
      </div>
      <button class="iconbtn dialog-close" data-close title="Fermer (Échap)">${icon('close', { size: 14 })}</button>
    </div>

    <div class="dialog-body">
      <!-- Sélecteur de format source -->
      <div class="import-format-bar">
        <span class="import-format-label">${icon('settings', { size: 13 })} Format source :</span>
        <div class="viewtabs" id="imp-format-tabs" role="tablist">
          <button type="button" class="active" data-format="auto" title="Détection automatique selon le contenu">⚡ Auto-détection</button>
          <button type="button" data-format="krello" title="Export JSON natif Krello">Krello</button>
          <button type="button" data-format="trello" title="Export de tableau Trello">Trello</button>
        </div>
      </div>

      <!-- Onglets mode d'importation -->
      <div class="import-tabs-row">
        <div class="viewtabs" id="imp-method-tabs" role="tablist">
          <button type="button" class="active" data-method="file">${icon('file', { size: 13 })} Fichier .json</button>
          <button type="button" data-method="text">${icon('edit', { size: 13 })} Coller le JSON</button>
        </div>
        <div class="muted" style="font-size:11.5px" id="imp-method-hint">Glissez un fichier ou parcourez</div>
      </div>

      <!-- Mode 1 : Fichier JSON -->
      <div id="imp-pane-file">
        <div class="import-dropzone" id="imp-dropzone" tabindex="0">
          <input type="file" id="imp-file-input" accept=".json,application/json,text/plain" style="display:none" />
          <div class="dropzone-icon">${icon('upload', { size: 22 })}</div>
          <div>
            <div class="dropzone-title">Glissez votre fichier <strong>.json</strong> ici</div>
            <div class="dropzone-sub">ou <button type="button" class="btn sm" id="imp-browse-btn" style="pointer-events:auto">Parcourir les fichiers</button></div>
          </div>
        </div>
        <div class="import-file-card" id="imp-file-card" style="display:none">
          <div class="file-card-icon">${icon('file', { size: 20 })}</div>
          <div class="file-card-details">
            <div class="file-card-name" id="imp-file-name">fichier.json</div>
            <div class="file-card-meta" id="imp-file-meta">0 Ko</div>
          </div>
          <button type="button" class="btn ghost sm" id="imp-file-clear" title="Choisir un autre fichier">Remplacer</button>
        </div>
      </div>

      <!-- Mode 2 : Coller le JSON -->
      <div id="imp-pane-text" style="display:none" class="import-text-pane">
        <div class="import-text-actions">
          <span class="muted" style="font-size:12px">Contenu brut du JSON :</span>
          <button type="button" class="btn ghost sm" id="imp-paste-clip" title="Coller depuis le presse-papier">${icon('copy', { size: 12 })} <span>Coller</span></button>
        </div>
        <textarea class="input" id="imp-json" style="min-height:140px;font-family:var(--mono);font-size:12px;line-height:1.5;resize:vertical" placeholder='Collez ici le JSON exporté depuis Krello ou Trello...'></textarea>
      </div>

      <!-- Aperçu de détection -->
      <div class="import-preview" id="imp-preview" style="display:none">
        <div class="preview-badge" id="imp-preview-badge">Krello</div>
        <div class="preview-content">
          <div class="preview-name" id="imp-preview-title">Tableau</div>
          <div class="preview-meta" id="imp-preview-meta">0 listes • 0 cartes</div>
        </div>
        <div class="preview-check" title="Valide">${icon('check', { size: 18 })}</div>
      </div>

      <!-- Message d'erreur -->
      <div class="import-error" id="imp-error" style="display:none">
        ${icon('alert', { size: 16 })}
        <span id="imp-error-text">Format non reconnu</span>
      </div>
    </div>

    <div class="dialog-footer">
      <div class="dialog-hints" id="imp-hints">
        <span class="kbd">JSON</span> Krello ou Trello
      </div>
      <div class="dialog-btns">
        <button type="button" class="btn" data-close>Annuler</button>
        <button type="button" class="btn primary" id="imp-submit" disabled>
          ${icon('import', { size: 14 })}
          <span id="imp-submit-text">Importer</span>
        </button>
      </div>
    </div>`, { size: 'md' });

  function inspectJson(text) {
    if (!text || !text.trim()) return null;
    let parsed;
    try {
      parsed = JSON.parse(text);
    } catch (err) {
      return { status: 'invalid', message: 'Syntaxe JSON invalide : ' + err.message };
    }
    if (!parsed || typeof parsed !== 'object') {
      return { status: 'invalid', message: 'Le JSON doit être un objet.' };
    }

    // Vérifier Krello (format 'krello' ou objet avec 'board' et 'lists')
    const isKrello = parsed.format === 'krello' || (parsed.board && typeof parsed.board === 'object' && Array.isArray(parsed.lists));
    if (isKrello) {
      const title = parsed.board?.title || 'Tableau Krello';
      const listsCount = Array.isArray(parsed.lists) ? parsed.lists.length : 0;
      const cardsCount = Array.isArray(parsed.cards) ? parsed.cards.length : 0;
      return {
        status: 'ok',
        format: 'krello',
        title,
        listsCount,
        cardsCount,
        label: 'Krello JSON',
      };
    }

    // Vérifier Trello ('name' et cartes/listes)
    const isTrello = typeof parsed.name === 'string' && (Array.isArray(parsed.cards) || Array.isArray(parsed.lists) || parsed.idBoard || parsed.id);
    if (isTrello) {
      const title = parsed.name || 'Tableau Trello';
      const listsCount = Array.isArray(parsed.lists) ? parsed.lists.filter((l) => !l.closed).length : 0;
      const cardsCount = Array.isArray(parsed.cards) ? parsed.cards.filter((c) => !c.closed).length : 0;
      return {
        status: 'ok',
        format: 'trello',
        title,
        listsCount,
        cardsCount,
        label: 'Export Trello',
      };
    }

    return {
      status: 'unrecognized',
      message: "Ce fichier n'est pas reconnu comme un export JSON Krello ou Trello.",
    };
  }

  function updateUI() {
    const result = inspectJson(rawText);
    const previewEl = el.querySelector('#imp-preview');
    const errorEl = el.querySelector('#imp-error');
    const submitBtn = el.querySelector('#imp-submit');
    const submitText = el.querySelector('#imp-submit-text');
    const hintsEl = el.querySelector('#imp-hints');

    if (!rawText.trim()) {
      previewEl.style.display = 'none';
      errorEl.style.display = 'none';
      submitBtn.disabled = true;
      submitText.textContent = 'Importer';
      hintsEl.innerHTML = '<span class="kbd">JSON</span> Krello ou Trello';
      return;
    }

    if (result.status === 'invalid') {
      previewEl.style.display = 'none';
      errorEl.style.display = 'flex';
      el.querySelector('#imp-error-text').textContent = result.message;
      submitBtn.disabled = true;
      submitText.textContent = 'Importer';
      hintsEl.innerHTML = '<span style="color:var(--error);font-weight:600">JSON invalide</span>';
      return;
    }

    const effectiveFormat = selectedFormat === 'auto' ? result.format : selectedFormat;

    if (result.status === 'ok') {
      errorEl.style.display = 'none';
      previewEl.style.display = 'flex';

      const badgeEl = el.querySelector('#imp-preview-badge');
      badgeEl.textContent = effectiveFormat === 'krello' ? 'Krello JSON' : 'Export Trello';
      badgeEl.className = effectiveFormat === 'trello' ? 'preview-badge trello' : 'preview-badge';

      el.querySelector('#imp-preview-title').textContent = result.title;
      el.querySelector('#imp-preview-meta').textContent =
        `${result.listsCount} liste${result.listsCount > 1 ? 's' : ''} • ${result.cardsCount} carte${result.cardsCount > 1 ? 's' : ''}`;

      submitBtn.disabled = false;
      const shortTitle = result.title.length > 24 ? result.title.slice(0, 22) + '…' : result.title;
      submitText.textContent = `Importer « ${shortTitle} »`;
      hintsEl.innerHTML = `<span style="color:var(--success);font-weight:600">✓ Prêt à importer (${effectiveFormat === 'krello' ? 'Krello' : 'Trello'})</span>`;
    } else {
      // Non reconnu automatiquement
      if (selectedFormat !== 'auto') {
        errorEl.style.display = 'none';
        previewEl.style.display = 'flex';
        const badgeEl = el.querySelector('#imp-preview-badge');
        badgeEl.textContent = selectedFormat === 'krello' ? 'Krello (forcé)' : 'Trello (forcé)';
        badgeEl.className = selectedFormat === 'trello' ? 'preview-badge trello' : 'preview-badge';
        el.querySelector('#imp-preview-title').textContent = 'Fichier JSON personnalisé';
        el.querySelector('#imp-preview-meta').textContent = 'Format forcé manuellement';
        submitBtn.disabled = false;
        submitText.textContent = selectedFormat === 'krello' ? 'Importer comme Krello' : 'Importer comme Trello';
        hintsEl.innerHTML = `<span class="badge">${selectedFormat.toUpperCase()}</span> Format forcé`;
      } else {
        previewEl.style.display = 'none';
        errorEl.style.display = 'flex';
        el.querySelector('#imp-error-text').textContent = result.message;
        submitBtn.disabled = true;
        submitText.textContent = 'Importer';
        hintsEl.innerHTML = '<span style="color:var(--warning);font-weight:600">Format non reconnu</span>';
      }
    }
  }

  function handleFile(file) {
    if (!file) return;
    selectedFileName = file.name;
    const kb = Math.round(file.size / 1024);
    selectedFileSize = kb > 1024 ? `${(kb / 1024).toFixed(1)} Mo` : `${kb} Ko`;

    const reader = new FileReader();
    reader.onload = (e) => {
      rawText = e.target.result;
      el.querySelector('#imp-json').value = rawText;

      el.querySelector('#imp-dropzone').style.display = 'none';
      el.querySelector('#imp-file-card').style.display = 'flex';
      el.querySelector('#imp-file-name').textContent = selectedFileName;
      el.querySelector('#imp-file-meta').textContent = selectedFileSize;

      updateUI();
    };
    reader.onerror = () => {
      toast('Erreur lors de la lecture du fichier', 'err');
    };
    reader.readAsText(file);
  }

  // --- Format Tabs ---
  const formatTabs = el.querySelectorAll('#imp-format-tabs button');
  formatTabs.forEach((btn) => {
    btn.onclick = () => {
      formatTabs.forEach((b) => b.classList.remove('active'));
      btn.classList.add('active');
      selectedFormat = btn.dataset.format;
      updateUI();
    };
  });

  // --- Method Tabs ---
  const methodTabs = el.querySelectorAll('#imp-method-tabs button');
  const paneFile = el.querySelector('#imp-pane-file');
  const paneText = el.querySelector('#imp-pane-text');
  const methodHint = el.querySelector('#imp-method-hint');

  methodTabs.forEach((btn) => {
    btn.onclick = () => {
      methodTabs.forEach((b) => b.classList.remove('active'));
      btn.classList.add('active');
      activeMethod = btn.dataset.method;
      if (activeMethod === 'file') {
        paneFile.style.display = 'block';
        paneText.style.display = 'none';
        methodHint.textContent = 'Glissez un fichier ou parcourez';
      } else {
        paneFile.style.display = 'none';
        paneText.style.display = 'flex';
        methodHint.textContent = 'Édition directe du JSON';
        setTimeout(() => el.querySelector('#imp-json').focus(), 50);
      }
    };
  });

  // --- File Dropzone & Input ---
  const dropzone = el.querySelector('#imp-dropzone');
  const fileInput = el.querySelector('#imp-file-input');
  const browseBtn = el.querySelector('#imp-browse-btn');
  const fileClearBtn = el.querySelector('#imp-file-clear');

  browseBtn.onclick = (e) => {
    e.stopPropagation();
    fileInput.click();
  };

  dropzone.onclick = () => fileInput.click();
  dropzone.onkeydown = (e) => {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      fileInput.click();
    }
  };

  fileInput.onchange = (e) => {
    const file = e.target.files[0];
    if (file) handleFile(file);
  };

  fileClearBtn.onclick = () => {
    fileInput.value = '';
    rawText = '';
    el.querySelector('#imp-json').value = '';
    el.querySelector('#imp-file-card').style.display = 'none';
    dropzone.style.display = 'flex';
    updateUI();
  };

  dropzone.ondragover = (e) => {
    e.preventDefault();
    dropzone.classList.add('drag-over');
  };
  dropzone.ondragleave = () => {
    dropzone.classList.remove('drag-over');
  };
  dropzone.ondrop = (e) => {
    e.preventDefault();
    dropzone.classList.remove('drag-over');
    const file = e.dataTransfer?.files?.[0];
    if (file) handleFile(file);
  };

  // --- Textarea & Clipboard ---
  const textarea = el.querySelector('#imp-json');
  textarea.oninput = (e) => {
    rawText = e.target.value;
    updateUI();
  };

  const pasteBtn = el.querySelector('#imp-paste-clip');
  pasteBtn.onclick = async () => {
    try {
      const text = await navigator.clipboard.readText();
      if (text) {
        rawText = text;
        textarea.value = text;
        updateUI();
        toast('Contenu du presse-papier collé', 'ok');
      } else {
        toast('Presse-papier vide', 'err');
      }
    } catch (_) {
      textarea.focus();
      toast('Collez directement avec ⌘V dans la zone de texte', 'err');
    }
  };

  // --- Submit Import ---
  const submitBtn = el.querySelector('#imp-submit');
  submitBtn.onclick = async () => {
    if (!rawText.trim()) return toast('Veuillez fournir un JSON à importer', 'err');

    const inspection = inspectJson(rawText);
    let targetFormat = selectedFormat;
    if (targetFormat === 'auto') {
      if (inspection?.status === 'ok') {
        targetFormat = inspection.format;
      } else {
        return toast("Impossible de déterminer le format d'importation", 'err');
      }
    }

    const importFn = targetFormat === 'trello' ? api.importTrello : api.importKrello;
    const submitLabel = el.querySelector('#imp-submit-text');
    const originalLabel = submitLabel.textContent;

    submitBtn.disabled = true;
    submitLabel.textContent = 'Importation en cours…';

    try {
      const board = await importFn(rawText);
      await loadBoards();
      el.closest('.modal-backdrop').remove();
      toast(`Board « ${board.title} » importé avec succès`, 'ok');
      navigate(boardUrl(board.id, 'board'));
    } catch (err) {
      submitBtn.disabled = false;
      submitLabel.textContent = originalLabel;
      toast(`Import impossible : ${err.message}`, 'err');
    }
  };
}

export function initSidebar(host) {
  renderSidebar(host);
  on('boards.changed', () => renderSidebar(host));
  on('board.loaded', () => renderSidebar(host));
  on('CardCreated', () => renderSidebar(host));
  on('CardDeleted', () => renderSidebar(host));
}
