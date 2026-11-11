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
};

export function getResolvedTheme() {
  if (state.theme === 'auto') {
    return window.matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark';
  }
  return state.theme;
}

export function applyStyle() {
  document.documentElement.dataset.style = state.style;
  localStorage.setItem('krello-style', state.style);
  if (state.board) {
    applyBoardBackground(state.board.background);
  }
}

export function applyTheme() {
  const resolved = getResolvedTheme();
  document.documentElement.dataset.theme = resolved;
  localStorage.setItem('krello-theme', state.theme);
  if (state.board) {
    applyBoardBackground(state.board.background);
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
  const [board, lists, cards, labels, fields, deps] = await Promise.all([
    api.board(boardId),
    api.lists(boardId),
    api.cards(boardId, true),
    api.labels(boardId),
    api.fields(boardId),
    api.deps(boardId),
  ]);
  let schedule = null;
  try { schedule = await api.schedule(boardId); } catch { /* pas de dates */ }
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

export function labelById(id) {
  return state.labels.find((l) => l.id === id);
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

export function cardsOfList(listId) {
  const { text, labelId, priority, due } = state.filters;
  let cards = state.cards.filter((c) => c.list_id === listId && (state.filters.includeArchived || !c.archived));
  if (text) {
    const t = text.toLowerCase();
    cards = cards.filter((c) => c.title.toLowerCase().includes(t) || c.description.toLowerCase().includes(t));
  }
  if (labelId) cards = cards.filter((c) => deps_hasLabel(c, labelId));
  if (priority) cards = cards.filter((c) => c.priority === priority);
  if (due === 'overdue') cards = cards.filter((c) => c.due && !c.due_complete && new Date(c.due) < new Date());
  if (due === 'week') {
    const limit = new Date(Date.now() + 7 * 86400000);
    cards = cards.filter((c) => c.due && !c.due_complete && new Date(c.due) <= limit);
  }
  cards.sort((a, b) => a.position - b.position);
  return cards;
}

function deps_hasLabel(card, labelId) {
  // card_labels n'est pas dans la liste des cards : utilisons card_detail lazy via labelsStore
  return (card._label_ids || []).includes(labelId);
}

/** Renseigne _label_ids sur chaque carte (une seule requête par board via détails). */
export async function hydrateLabelIds() {
  // Le endpoint /cards ne renvoie pas les labels ; on récupère les paires via une
  // requête par carte serait coûteux : on passe par card_detail seulement à l'ouverture.
  // Pour le filtrage instantané, on marque les cartes via labels du board + detail cache.
  if (!state.board) return;
  const details = await Promise.allSettled(state.cards.map((c) => api.card(c.id)));
  state.cards.forEach((c, i) => {
    const d = details[i];
    if (d.status === 'fulfilled') {
      c._label_ids = (d.value.labels || []).map((l) => l.id);
      c._checklists = d.value.checklists || [];
      c._comments_count = (d.value.comments || []).length;
      c._attachments_count = (d.value.attachments || []).length;
    }
  });
  emit('board.hydrated');
}

export function visibleLists() {
  return state.lists
    .filter((l) => state.filters.includeArchived || !l.archived)
    .sort((a, b) => a.position - b.position);
}
