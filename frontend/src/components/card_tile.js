// Tuile de carte Kanban + drag & drop.

import { escapeHtml, dueBadge, dueState, priorityBadge, fmtDateShort, progressPct } from '../format.js';
import { draggable } from '../dnd.js';
import { state } from '../state.js';
import { openCardModal } from './card_modal.js';

export function cardTileHtml(card) {
  const labels = (card._label_ids || [])
    .map((id) => state.labels.find((l) => l.id === id))
    .filter(Boolean);
  const pct = card._checklists ? progressPct(card._checklists) : null;
  const cover = card.cover ? safeCover(card.cover) : '';
  const milestone = card.is_milestone ? '<span class="mini">◆ jalon</span>' : '';
  const duration = card.duration_days ? `<span class="mini">⏳ ${card.duration_days} j</span>` : '';
  const counts = [
    card._comments_count ? `<span class="mini">💬 ${card._comments_count}</span>` : '',
    card._attachments_count ? `<span class="mini">📎 ${card._attachments_count}</span>` : '',
  ].join('');
  return `
    <div class="card-tile" data-card="${card.id}">
      ${cover}
      <div class="title ${card.due_complete ? 'done' : ''}">${escapeHtml(card.title)}</div>
      <div class="meta">
        ${labels.slice(0, 4).map((l) => `<span class="chip-label" style="background:${l.color}" title="${escapeHtml(l.name)}">${escapeHtml(l.name || '•')}</span>`).join('')}
        ${dueBadge(card)}
        ${priorityBadge(card)}
        ${milestone}
        ${duration}
        ${pct !== null ? `<span class="mini">☑ ${pct}%</span>` : ''}
        ${counts}
      </div>
    </div>`;
}

function safeCover(coverJson) {
  try {
    const c = JSON.parse(coverJson);
    if (c.type === 'color') return `<div class="cover" style="background:${escapeHtml(c.value)}"></div>`;
  } catch { /* ignore */ }
  return '';
}

export function attachCardTile(el, card, { onChange }) {
  el.addEventListener('click', () => openCardModal(card.id, { onChange }));

  draggable(el, {
    kind: 'card',
    id: card.id,
    targetSelector: '.list',
    canDrag: (ev) => !ev.target.closest('button, input, textarea, a'),
    onDrop: async ({ targetEl }) => {
      const listEl = targetEl.closest('.list');
      const targetListId = listEl?.dataset.list;
      if (!targetListId) return;
      // position : carte avant laquelle insérer (position du pointeur)
      const tiles = [...listEl.querySelectorAll('.card-tile')].filter((t) => t.dataset.card !== card.id);
      const beforeId = tiles.find((t) => t.getBoundingClientRect().bottom > lastY)?.dataset.card || null;
      try {
        await window.__krelloMoveCard(card.id, { list_id: targetListId, before: beforeId || undefined });
        onChange?.();
      } catch (e) {
        import('../ui/modal.js').then(({ toast }) => toast(e.message, 'err'));
      }
    },
  });
}

let lastY = 0;
window.addEventListener('pointermove', (e) => { lastY = e.clientY; });
