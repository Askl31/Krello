// Helpers partagés par les vues.

import { api } from '../api.js';
import { state } from '../state.js';

export async function saveViewSettings(view, settings) {
  const merged = { ...(state.settings[view] || {}), ...settings };
  state.settings[view] = merged;
  if (state.board) {
    try { await api.saveViewSettings(state.board.id, view, merged); } catch { /* best-effort */ }
  }
}

export function loadViewSettings(allSettings) {
  state.settings = allSettings || {};
}
