// Tuile de carte Kanban + drag & drop.

import { escapeHtml, dueBadge, priorityBadge, progressPct } from '../format.js';
import { draggable } from '../dnd.js';
import { state } from '../state.js';
import { openCardModal } from './card_modal.js';
import { icon } from '../icons.js';

export function cardTileHtml(card) {
  const labels = (card._label_ids || [])
    .map((id) => state.labels.find((l) => l.id === id))
    .filter(Boolean);
  const pct = card._checklists ? progressPct(card._checklists) : null;
  const cover = card.cover ? safeCover(card.cover) : '';
  const milestone = card.is_milestone
    ? `<span class="badge" style="color:var(--warning);background:var(--warning-soft);border-color:color-mix(in srgb,var(--warning)35%,transparent)">${icon('milestone', { size: 10 })} jalon</span>`
    : '';
  const templateBadge = card.is_template
    ? `<span class="badge" style="color:var(--accent);background:var(--accent-soft);border-color:color-mix(in srgb,var(--accent)35%,transparent)">${icon('template', { size: 10 })} modèle</span>`
    : '';
  const duration = card.duration_days
    ? `<span class="mini" title="Durée estimée : ${card.duration_days} jours ouvrés">${icon('clock', { size: 11 })} ${card.duration_days}j</span>`
    : '';
  const counts = [
    card._comments_count ? `<span class="mini" title="${card._comments_count} commentaire(s)">${icon('comment', { size: 11 })} ${card._comments_count}</span>` : '',
    card._attachments_count ? `<span class="mini" title="${card._attachments_count} pièce(s) jointe(s)">${icon('attachment', { size: 11 })} ${card._attachments_count}</span>` : '',
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
        ${templateBadge}
        ${duration}
        ${pct !== null ? `<span class="badge" style="color:var(--success);background:var(--success-soft);border-color:color-mix(in srgb,var(--success)35%,transparent)">${icon('check', { size: 11 })} ${pct}%</span>` : ''}
        ${counts}
      </div>
    </div>`;
}

function safeCover(coverJson) {
  try {
    const c = JSON.parse(coverJson);
    if (c.type === 'color') return `<div class="cover" style="background:${escapeHtml(c.value)}"></div>`;
    if (c.type === 'attachment' && c.value) {
      return `<img class="cover" src="/api/v1/attachments/${encodeURIComponent(c.value)}/download" alt="" loading="lazy" />`;
    }
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

export function updateCardTileInDOM(cardId, { onChange } = {}) {
  const tile = document.querySelector(`.card-tile[data-card="${cardId}"]`);
  if (!tile) return;
  const card = state.cards.find((c) => c.id === cardId);
  if (!card) return;
  const temp = document.createElement('div');
  temp.innerHTML = cardTileHtml(card);
  const newTile = temp.firstElementChild;
  if (!newTile) return;
  if (tile.classList.contains('keyboard-focused')) {
    newTile.classList.add('keyboard-focused');
  }
  attachCardTile(newTile, card, { onChange: onChange || window.__krelloReloadBoard });
  tile.replaceWith(newTile);
}

let lastY = 0;
window.addEventListener('pointermove', (e) => { lastY = e.clientY; });
window.__krelloUpdateCardTile = updateCardTileInDOM;
