// Bootstrap Krello : shell, router, WS, raccourcis clavier.

import { api } from './api.js';
import { startWs, on, emit } from './ws.js';
import { state, applyTheme, loadBoards, loadBoard, hydrateLabelIds } from './state.js';
import { startRouter, boardUrl, navigate } from './router.js';
import { initPalette } from './ui/palette.js';
import { initTopbarShortcuts, renderTopbar } from './components/topbar.js';
import { initSidebar, renderSidebar } from './components/sidebar.js';
import { renderBoard } from './views/board.js';
import { renderCalendar } from './views/calendar.js';
import { renderTimeline } from './views/timeline.js';
import { renderTable } from './views/table.js';
import { renderDashboard } from './views/dashboard.js';
import { renderSettings } from './views/settings.js';
import { loadViewSettings } from './views/view_helpers.js';

const app = document.getElementById('app');
applyTheme();

// Statut IA une seule fois au boot : pilote l'affichage des boutons ✨.
try {
  state.aiStatus = await api.aiStatus();
} catch {
  state.aiStatus = { available: false, reason: 'unreachable' };
}

function shellHtml() {
  return `
    <div class="topbar" id="topbar"></div>
    <div class="shell">
      <div class="sidebar" id="sidebar"></div>
      <div class="main" id="main"><div class="view"></div></div>
    </div>`;
}

async function renderRoute() {
  const route = location.hash;
  if (!app.querySelector('.shell')) {
    app.innerHTML = shellHtml();
    initSidebar(document.getElementById('sidebar'));
    initTopbarShortcuts();
  }
  await loadBoards();

  if (route.startsWith('#/b/')) {
    const [, , boardId, view = 'board'] = route.split('/');
    try {
      // Toujours recharger : les événements WS et les mutations locales
      // doivent refléter l'état serveur (listes, cartes, deps, schedule).
      await loadBoard(boardId);
      const allSettings = {};
      try {
        const views = await api.get(`/api/v1/boards/${boardId}/views`);
        for (const v of views) allSettings[v.view] = JSON.parse(v.settings || '{}');
      } catch { /* optionnel */ }
      loadViewSettings(allSettings);
    } catch (e) {
      document.getElementById('main').innerHTML =
        `<div class="empty-state"><div class="big">🌫</div><p>Board introuvable : ${e.message}</p></div>`;
      return;
    }
    state.view = view;
    await hydrateLabelIds();
    renderTopbar(document.getElementById('topbar'));
    const main = document.getElementById('main');
    main.innerHTML = '<div class="view"></div>';
    switch (view) {
      case 'calendar': renderCalendar(); break;
      case 'timeline': renderTimeline(); break;
      case 'table': renderTable(); break;
      case 'dashboard': renderDashboard(); break;
      default: renderBoard();
    }
  } else if (route === '#/settings') {
    state.board = null;
    renderTopbar(document.getElementById('topbar'));
    document.getElementById('main').innerHTML = '<div class="view"></div>';
    renderSettings();
  } else {
    // accueil
    state.board = null;
    renderTopbar(document.getElementById('topbar'));
    const main = document.getElementById('main');
    if (state.boards.length) {
      navigate(boardUrl(state.boards[0].id, 'board'));
      return;
    }
    main.innerHTML = `
      <div class="empty-state">
        <div class="big">👋</div>
        <h2 style="margin:0 0 6px">Bienvenue dans Krello</h2>
        <p>Kanban local avec Gantt/CPM, calendrier, automatisations et Apple Intelligence.<br/>
        Créez votre premier board avec <span class="kbd">⌘K</span> → « Nouveau board ».</p>
        <button class="btn primary" id="home-new">＋ Créer un board</button>
      </div>`;
    main.querySelector('#home-new').onclick = async () => {
      const { promptDialog } = await import('./ui/modal.js');
      const name = await promptDialog('Nom du board', 'Mon premier board');
      if (!name) return;
      const b = await api.createBoard({ title: name });
      await loadBoards();
      navigate(boardUrl(b.id, 'board'));
    };
  }
}

// Réactivité WS : rechargement ciblé selon les événements serveur.
let reloadTimer = null;
function scheduleReload() {
  clearTimeout(reloadTimer);
  reloadTimer = setTimeout(() => renderRoute(), 150);
}

on('*', (evt) => {
  const t = evt.type;
  if (['CardCreated', 'CardUpdated', 'CardMoved', 'CardDeleted', 'LabelsUpdated',
    'ChecklistsUpdated', 'CommentsUpdated', 'AttachmentsUpdated', 'FieldsUpdated',
    'DependenciesUpdated', 'ScheduleUpdated', 'UndoApplied', 'resync',
    'ListCreated', 'ListUpdated', 'ListDeleted', 'BoardUpdated'].includes(t)) {
    scheduleReload();
  } else if (t === 'BoardDeleted' || t === 'BoardCreated') {
    scheduleReload();
  } else if (t === 'AutomationsUpdated' || t === 'BaselinesUpdated') {
    scheduleReload();
  }
});

emit('app.boot');
startWs();
initPalette();
startRouter(renderRoute);
