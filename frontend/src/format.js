// Formatage FR + utilitaires de rendu.

import { icon } from './icons.js';

export function escapeHtml(s) {
  return String(s ?? '')
    .replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;').replaceAll("'", '&#39;');
}

/** Markdown sûr par allowlist : tout le HTML est d'abord échappé, puis les
 * règles reconnaissent : titres #..###, citations >, blocs ```, code inline,
 * gras, italique, barré ~~..~~, listes -, liens [texte](url) limités aux
 * protocoles http/https/mailto (javascript:/data: refusés). */
export function mdToHtml(src) {
  const esc = escapeHtml(src ?? '');

  // 1. Blocs de code ``` extraits d'abord (protégés des règles inline).
  const blocks = [];
  let s = esc.replace(/```([\s\S]*?)```/g, (_, code) => {
    blocks.push(`<pre class="md-code">${code.replace(/^\n/, '')}</pre>`);
    return `\u0000BLOCK${blocks.length - 1}\u0000`;
  });

  // 2. Titres (ligne entière)
  s = s.replace(/^# (.+)$/gm, '<h3>$1</h3>');
  s = s.replace(/^## (.+)$/gm, '<h4>$1</h4>');
  s = s.replace(/^### (.+)$/gm, '<h5>$1</h5>');

  // 3. Citations : '>' est déjà échappé en &gt; après escapeHtml
  s = s.replace(/^&gt; (.+)$/gm, '<blockquote>$1</blockquote>');

  // 4. Inline : code, gras, italique, barré
  s = s.replace(/`([^`]+)`/g, '<code>$1</code>');
  s = s.replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>');
  s = s.replace(/(^|\W)\*([^*\n]+)\*(?=\W|$)/g, '$1<em>$2</em>');
  s = s.replace(/~~([^~\n]+)~~/g, '<del>$1</del>');

  // 5. Liens : protocole restreint, URL déjà échappée (pas de " possible)
  s = s.replace(/\[([^\]]+)\]\(([^)\s]+)\)/g, (whole, text, url) => {
    if (!/^(https?:\/\/|mailto:)/i.test(url)) return text; // protocole refusé → texte seul
    return `<a href="${url}" target="_blank" rel="noopener noreferrer">${text}</a>`;
  });

  // 6. Listes
  s = s.replace(/^- (.+)$/gm, '<li>$1</li>');
  s = s.replace(/(<li>[\s\S]*?<\/li>)/g, '<ul>$1</ul>');

  // 7. Retours à la ligne
  s = s.replace(/\n/g, '<br>');

  // 8. Restaurer les blocs de code
  s = s.replace(/\u0000BLOCK(\d+)\u0000/g, (_, i) => `<br>${blocks[Number(i)]}<br>`);
  return s;
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
