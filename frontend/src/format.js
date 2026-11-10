// Formatage FR + utilitaires de rendu.

import { icon } from './icons.js';

export function escapeHtml(s) {
  return String(s ?? '')
    .replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;').replaceAll("'", '&#39;');
}

/** Markdown minimal sûr : gras, italique, code, listes, retours ligne. */
export function mdToHtml(src) {
  const esc = escapeHtml(src ?? '');
  return esc
    .replace(/`([^`]+)`/g, '<code>$1</code>')
    .replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>')
    .replace(/(^|\W)\*([^*]+)\*(?=\W|$)/g, '$1<em>$2</em>')
    .replace(/^- (.+)$/gm, '<li>$1</li>')
    .replace(/(<li>[\s\S]*?<\/li>)/g, '<ul>$1</ul>')
    .replace(/\n/g, '<br>');
}

const DAYS = ['dim.', 'lun.', 'mar.', 'mer.', 'jeu.', 'ven.', 'sam.'];
const MONTHS = ['janvier', 'février', 'mars', 'avril', 'mai', 'juin', 'juillet',
  'août', 'septembre', 'octobre', 'novembre', 'décembre'];

export function parseDate(s) {
  if (!s) return null;
  if (/^\d{4}-\d{2}-\d{2}$/.test(s)) {
    const [y, m, d] = s.split('-').map(Number);
    return new Date(y, m - 1, d);
  }
  const d = new Date(s);
  return isNaN(d) ? null : d;
}

export function fmtDate(s) {
  const d = parseDate(s);
  if (!d) return '';
  return `${DAYS[d.getDay()]} ${d.getDate()} ${MONTHS[d.getMonth()]}`;
}

export function fmtDateShort(s) {
  const d = parseDate(s);
  if (!d) return '';
  return `${d.getDate()} ${MONTHS[d.getMonth()].slice(0, 4)}.`;
}

export function fmtDateTime(s) {
  const d = parseDate(s);
  if (!d) return '';
  const hasTime = !/^\d{4}-\d{2}-\d{2}$/.test(String(s ?? ''));
  return fmtDate(s) + (hasTime ? ` · ${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}` : '');
}

export function toISODate(d) {
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, '0');
  const day = String(d.getDate()).padStart(2, '0');
  return `${y}-${m}-${day}`;
}

export function dueState(card, today = new Date()) {
  if (!card.due || card.due_complete) return null;
  const d = parseDate(card.due);
  if (!d) return null;
  const t0 = new Date(today.getFullYear(), today.getMonth(), today.getDate());
  if (d < t0) return 'overdue';
  const diff = (d - t0) / 86400000;
  return diff <= 2 ? 'due-soon' : null;
}

export const PRIORITY_META = {
  none: { label: '—', color: 'var(--muted)' },
  low: { label: 'basse', color: 'var(--c4)' },
  medium: { label: 'moyenne', color: 'var(--c3)' },
  high: { label: 'haute', color: 'var(--error)' },
  urgent: { label: 'urgente', color: 'var(--critical)' },
};

export const LABEL_COLORS = [
  '#ef4444', '#f59e0b', '#22c55e', '#14b8a6', '#3b82f6',
  '#6366f1', '#a855f7', '#ec4899', '#64748b', '#0ea5e9',
];

export function priorityBadge(card) {
  const m = PRIORITY_META[card.priority] || PRIORITY_META.none;
  if (card.priority === 'none') return '';
  return `<span class="badge" style="color:${m.color};border-color:color-mix(in srgb, ${m.color} 35%, transparent);background:color-mix(in srgb, ${m.color} 10%, transparent)">${icon('arrowUp', { size: 10 })} ${m.label}</span>`;
}

export function dueBadge(card) {
  const state = dueState(card);
  if (!card.due) return '';
  const cls = card.due_complete ? 'done' : state === 'overdue' ? 'overdue' : state === 'due-soon' ? 'due-soon' : '';
  const ic = card.due_complete ? icon('check', { size: 11 }) : icon('clock', { size: 11 });
  return `<span class="badge ${cls}">${ic} <span>${fmtDateShort(card.due)}</span></span>`;
}

export function progressPct(checklists) {
  let total = 0, done = 0;
  for (const cl of checklists) {
    for (const it of cl.items) {
      total += 1;
      if (it.checked) done += 1;
    }
  }
  return total ? Math.round((done / total) * 100) : null;
}
