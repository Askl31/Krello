// Tuile de carte Kanban + drag & drop.

import { escapeHtml, dueBadge, priorityBadge, progressPct } from '../format.js';
import { state, labelById } from '../state.js';
import { icon } from '../icons.js';

export function cardTileHtml(card, opts = {}) {
  const labels = (card._label_ids || [])
    .map((id) => labelById(id))
    .filter(Boolean);
  const pct = card._checklists
    ? progressPct(card._checklists)
    : card._checklist_total
      ? Math.round((100 * card._checklist_done) / card._checklist_total)
      : null;
  const cover = card.cover ? safeCover(card.cover) : '';
  const hasCover = !!cover;
  const selected = !!opts.selected;
  const depth = Number(opts.depth) || 0;
  const subBadge = opts.subLabel
    ? `<span class="badge" style="color:var(--muted);background:var(--surface);border-color:var(--border)">${icon('chevronDown', { size: 10 })} ${escapeHtml(opts.subLabel)}</span>`
    : '';
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
    <div class="card-tile ${hasCover ? 'has-cover' : ''} ${selected ? 'selected' : ''} ${depth ? 'subcard' : ''}" data-card="${card.id}"${depth ? ` style="margin-left:${Math.min(depth, 4) * 18}px"` : ''}>
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
        ${subBadge}
      </div>
    </div>`;
}

function safeCover(coverJson) {
  try {
    const c = JSON.parse(coverJson);
    if (c.type === 'color' && c.value) return `<div class="cover" style="background:${escapeHtml(c.value)}"></div>`;
    if (c.type === 'attachment' && c.value) {
      return `<img class="cover" src="/api/v1/attachments/${encodeURIComponent(c.value)}/download" alt="" loading="lazy" />`;
    }
  } catch { /* ignore */ }
  return '';
}

export function attachCardTile(_el, _card, _opts = {}) {
  // Les événements des tuiles (clic/sélection, drag, drop de fichiers) sont
  // délégués sur document dans views/board.js : plus aucun listener par tuile,
  // donc rien à attacher ici — les tuiles rendues par innerHTML sont couvertes.
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

window.__krelloUpdateCardTile = updateCardTileInDOM;
