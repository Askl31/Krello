// État applicatif minimal + rechargement des données du board courant.

import { api } from './api.js';
import { emit } from './ws.js';

export const state = {
  boards: [],
  aiStatus: null,
  board: null,        // board courant
  lists: [],
  cards: [],
  labels: [],
  fields: [],
  deps: [],
  schedule: null,     // ScheduleSummary
  settings: {},       // board_views settings (json par vue)
  view: 'board',      // board | calendar | timeline | table | dashboard
  filters: { text: '', labelId: null, priority: null, due: null, includeArchived: false },
  theme: localStorage.getItem('krello-theme') || 'auto',
  style: localStorage.getItem('krello-style') || 'standard',
  settingsTab: 'appearance',
};

export function getResolvedTheme() {
  if (state.theme === 'auto') {
    return window.matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark';
  }
  return state.theme;
}

export function applyStyle(persistBackend = false) {
  document.documentElement.dataset.style = state.style;
  try {
    localStorage.setItem('krello-style', state.style);
  } catch { /* ignore */ }
  if (persistBackend) {
    api.saveUiSettings({ style: state.style }).catch(() => {});
  }
  if (state.board) {
    applyBoardBackground(state.board.background);
  }
}

export function applyTheme(persistBackend = false) {
  const resolved = getResolvedTheme();
  document.documentElement.dataset.theme = resolved;
  try {
    localStorage.setItem('krello-theme', state.theme);
  } catch { /* ignore */ }
  if (persistBackend) {
    api.saveUiSettings({ theme: state.theme }).catch(() => {});
  }
  if (state.board) {
    applyBoardBackground(state.board.background);
  }
}

export async function initUiSettings() {
  try {
    const res = await api.uiSettings();
    let themeChanged = false;
    let styleChanged = false;
    if (res.theme && res.theme !== state.theme) {
      state.theme = res.theme;
      try { localStorage.setItem('krello-theme', state.theme); } catch { /* ignore */ }
      themeChanged = true;
    }
    if (res.style && res.style !== state.style) {
      state.style = res.style;
      try { localStorage.setItem('krello-style', state.style); } catch { /* ignore */ }
      styleChanged = true;
    }
    if (res.sidebar_collapsed != null && localStorage.getItem('krello-sidebar-collapsed') == null) {
      try { localStorage.setItem('krello-sidebar-collapsed', res.sidebar_collapsed ? 'true' : 'false'); } catch { /* ignore */ }
    }
    if (themeChanged) applyTheme(false);
    if (styleChanged) applyStyle(false);
  } catch {
    // Mode hors-ligne / fallback local
  }
}

if (typeof window !== 'undefined') {
  window.__krelloState = state;
  window.__krelloApplyStyle = applyStyle;
  window.__krelloApplyTheme = applyTheme;
}

// Détection dynamique du changement d'apparence système macOS
if (window.matchMedia) {
  window.matchMedia('(prefers-color-scheme: light)').addEventListener('change', () => {
    if (state.theme === 'auto') applyTheme();
  });
}

export function applyBoardBackground(bg) {
  const main = document.getElementById('main');
  if (!main) return;
  const isLight = document.documentElement.dataset.theme === 'light';
  const isTranslucent = state.style === 'translucent';

  if (!bg) {
    main.style.background = '';
    return;
  }
  try {
    const parsed = typeof bg === 'string' ? JSON.parse(bg) : bg;
    // Si c'est la valeur par défaut du backend (#101828) ou "default" :
    // en mode translucide ou clair, on laisse transparent pour laisser transparaître le fond aurore / studio
    if (parsed.value === '#101828' || parsed.type === 'default' || !parsed.value) {
      main.style.background = (isTranslucent || isLight) ? '' : '#101828';
      return;
    }
    if (parsed.type === 'color' || parsed.type === 'gradient') {
      main.style.background = parsed.value;
      return;
    }
  } catch { /* ignore */ }

  if (typeof bg === 'string') {
    if (bg === '#101828') {
      main.style.background = (isTranslucent || isLight) ? '' : '#101828';
      return;
    }
    if (bg.startsWith('#') || bg.startsWith('linear-gradient')) {
      main.style.background = bg;
    }
  }
}

export async function loadBoards() {
  state.boards = await api.boards();
  emit('boards.changed');
}

export async function loadBoard(boardId) {
  // schedule démarre en parallèle des 6 autres requêtes (évite un RTT séquentiel).
  const scheduleP = api.schedule(boardId).catch(() => null);
  const [board, lists, cards, labels, fields, deps, schedule] = await Promise.all([
    api.board(boardId),
    api.lists(boardId),
    api.cards(boardId, true),
    api.labels(boardId),
    api.fields(boardId),
    api.deps(boardId),
    scheduleP,
  ]);
  Object.assign(state, { board, lists, cards, labels, fields, deps, schedule });
  applyBoardBackground(board.background);
  emit('board.loaded', boardId);
}

export async function refreshSchedule() {
  if (!state.board) return;
  try { state.schedule = await api.recompute(state.board.id); } catch { state.schedule = null; }
  emit('board.loaded');
}

export async function refreshBoard() {
  if (!state.board) return;
  await loadBoard(state.board.id);
  await hydrateLabelIds();
}

export function cardById(id) {
  return state.cards.find((c) => c.id === id);
}

export function listById(id) {
  return state.lists.find((l) => l.id === id);
}

// Cache mémoïsé des labels par id — invalidé automatiquement quand loadBoard
// réassigne state.labels (WeakMap indexée sur le tableau).
const labelsMapCache = new WeakMap();
export function labelsMap() {
  let m = labelsMapCache.get(state.labels);
  if (!m) {
    m = new Map(state.labels.map((l) => [l.id, l]));
    labelsMapCache.set(state.labels, m);
  }
  return m;
}

export function labelById(id) {
  return labelsMap().get(id);
}

export function scheduleFor(cardId) {
  return state.schedule?.cards.find((e) => e.card_id === cardId) || null;
}

/** Date effective de début/fin d'une carte : schedule CPM sinon dates manuelles. */
export function effectiveDates(card) {
  const s = scheduleFor(card.id);
  const start = s?.es || card.start || null;
  const end = s?.ef || card.due || null;
  return { start, end, critical: !!s?.is_critical, float: s?.total_float_days ?? null };
}

const EMPTY_CARDS = [];
// Cache mémoïsé du groupement cartes filtrées par liste — invalidé quand
// loadBoard réassigne state.cards (WeakMap) ou quand les filtres changent.
const byListCache = new WeakMap();
export function cardsByList() {
  const f = state.filters;
  const sig = JSON.stringify(f);
  let entry = byListCache.get(state.cards);
  if (!entry || entry.sig !== sig) {
    let cards = state.cards.filter((c) => f.includeArchived || !c.archived);
    if (f.text) {
      const t = f.text.toLowerCase();
      cards = cards.filter((c) => c.title.toLowerCase().includes(t) || c.description.toLowerCase().includes(t));
    }
    if (f.labelId) cards = cards.filter((c) => deps_hasLabel(c, f.labelId));
    if (f.priority) cards = cards.filter((c) => c.priority === f.priority);
    if (f.due === 'overdue') cards = cards.filter((c) => c.due && !c.due_complete && new Date(c.due) < new Date());
    if (f.due === 'week') {
      const limit = new Date(Date.now() + 7 * 86400000);
      cards = cards.filter((c) => c.due && !c.due_complete && new Date(c.due) <= limit);
    }
    const map = new Map();
    for (const c of cards) {
      let arr = map.get(c.list_id);
      if (!arr) { arr = []; map.set(c.list_id, arr); }
      arr.push(c);
    }
    for (const arr of map.values()) arr.sort((a, b) => a.position - b.position);
    entry = { sig, map };
    byListCache.set(state.cards, entry);
  }
  return entry.map;
}

export function cardsOfList(listId) {
  return cardsByList().get(listId) || EMPTY_CARDS;
}

function deps_hasLabel(card, labelId) {
  // card_labels n'est pas dans la liste des cards : utilisons card_detail lazy via labelsStore
  return (card._label_ids || []).includes(labelId);
}

/** Renseigne _label_ids + compteurs sur chaque carte via UN appel /cards/meta. */
export async function hydrateLabelIds() {
  if (!state.board) return;
  try {
    const meta = await api.cardsMeta(state.board.id);
    for (const c of state.cards) {
      const m = meta[c.id];
      if (!m) continue;
      c._label_ids = m.label_ids || [];
      c._checklists_count = m.checklists_count || 0;
      c._checklist_total = m.checklist_items || 0;
      c._checklist_done = m.checklist_done || 0;
      c._comments_count = m.comments_count || 0;
      c._attachments_count = m.attachments_count || 0;
    }
  } catch { /* métadonnées non hydratées : les tuiles restent fonctionnelles */ }
  emit('board.hydrated');
}

export function visibleLists() {
  return state.lists
    .filter((l) => state.filters.includeArchived || !l.archived)
    .sort((a, b) => a.position - b.position);
}
