// Toasts + modales génériques avec transitions douces.

import { escapeHtml } from '../format.js';
import { icon } from '../icons.js';

export function toast(message, kind = '') {
  let host = document.querySelector('.toasts');
  if (!host) {
    host = document.createElement('div');
    host.className = 'toasts';
    document.getElementById('overlays').appendChild(host);
  }
  const el = document.createElement('div');
  el.className = `toast ${kind}`;
  
  let ic = '';
  if (kind === 'ok') ic = icon('check', { size: 14, className: 'toast-icon' });
  else if (kind === 'err') ic = icon('close', { size: 14, className: 'toast-icon' });
  else if (kind === 'ai') ic = icon('ai', { size: 14, className: 'toast-icon' });

  el.innerHTML = `${ic}<span>${escapeHtml(message)}</span>`;
  host.appendChild(el);

  setTimeout(() => {
    el.style.opacity = '0';
    el.style.transform = 'translateY(10px) scale(0.96)';
    el.style.transition = 'opacity 0.25s ease, transform 0.25s ease';
    setTimeout(() => el.remove(), 260);
  }, 3200);
}

export function openModal(html, { onClose, id, size = 'md' } = {}) {
  // Une modale avec le même id réutilise l'élément existant au lieu de le détruire
  // pour éviter tout flash / re-trigger des animations CSS.
  let backdrop = id ? document.querySelector(`.modal-backdrop[data-modal-id="${id}"]`) : null;
  let modalEl;
  let isUpdate = false;

  if (backdrop) {
    isUpdate = true;
    modalEl = backdrop.querySelector('.modal');
    if (modalEl) {
      modalEl.className = `modal modal-${size} modal-updating`;
      modalEl.innerHTML = html;
    }
  } else {
    backdrop = document.createElement('div');
    backdrop.className = 'modal-backdrop';
    if (id) backdrop.dataset.modalId = id;
    backdrop.innerHTML = `<div class="modal modal-${size}">${html}</div>`;
    document.getElementById('overlays').appendChild(backdrop);
    modalEl = backdrop.querySelector('.modal');
  }

  backdrop.__krelloOnClose = onClose;

  const close = () => {
    backdrop.style.opacity = '0';
    backdrop.style.transition = 'opacity 0.18s ease';
    const modal = backdrop.querySelector('.modal');
    if (modal) {
      modal.style.transform = 'scale(0.96) translateY(6px)';
      modal.style.transition = 'transform 0.18s ease';
    }
    setTimeout(() => {
      backdrop.remove();
      backdrop.__krelloOnClose?.();
    }, 180);
  };

  if (!isUpdate) {
    backdrop.addEventListener('pointerdown', (e) => {
      if (e.target === backdrop) close();
    });

    const esc = (e) => {
      if (e.key === 'Escape') {
        close();
        document.removeEventListener('keydown', esc);
      }
    };
    document.addEventListener('keydown', esc);
  }

  backdrop.querySelectorAll('[data-close]').forEach((b) => b.addEventListener('click', close));

  return { el: modalEl, close };
}

export function confirmDialog(message, opts = {}) {
  const m = message.toLowerCase();
  const isDestructive = opts.isDanger !== undefined
    ? opts.isDanger
    : (m.includes('supprimer') || m.includes('delete') || m.includes('retirer') || m.includes('effacer') || m.includes('archiver'));

  const iconName = opts.icon || (isDestructive ? 'trash' : 'alert');
  const badgeClass = isDestructive ? 'danger' : 'warning';
  const confirmLabel = opts.confirmLabel || (isDestructive ? 'Supprimer' : 'Confirmer');
  const cancelLabel = opts.cancelLabel || 'Annuler';
  const desc = opts.desc || (isDestructive ? 'Cette opération est irréversible.' : '');

  return new Promise((resolve) => {
    const { el, close } = openModal(`
      <div class="dialog-header">
        <div class="dialog-badge ${badgeClass}">${icon(iconName, { size: 18 })}</div>
        <div class="dialog-titles">
          <h3 class="dialog-title">${escapeHtml(message)}</h3>
          ${desc ? `<p class="dialog-desc">${escapeHtml(desc)}</p>` : ''}
        </div>
        <button class="iconbtn dialog-close" data-close title="Fermer (Échap)">${icon('close', { size: 14 })}</button>
      </div>
      <div class="dialog-footer">
        <div class="dialog-hints">
          <span class="kbd">Échap</span> Annuler
        </div>
        <div class="dialog-btns">
          <button class="btn" data-act="no">${escapeHtml(cancelLabel)}</button>
          <button class="btn ${isDestructive ? 'danger' : 'primary'}" data-act="yes">${escapeHtml(confirmLabel)}</button>
        </div>
      </div>`, { size: 'sm' });

    el.querySelector('[data-act=no]').onclick = () => { close(); resolve(false); };
    el.querySelector('[data-act=yes]').onclick = () => { close(); resolve(true); };
    const prev = el.closest('.modal-backdrop');
    const obs = new MutationObserver(() => {
      if (!prev.isConnected) { resolve(false); obs.disconnect(); }
    });
    obs.observe(document.getElementById('overlays'), { childList: true });
  });
}

export function promptDialog(title, value = '', placeholder = '', opts = {}) {
  const t = title.toLowerCase();
  let defaultIcon = 'edit';
  let desc = opts.desc || '';
  let isNumber = opts.isNumber || false;
  let submitLabel = opts.submitLabel || 'Valider';

  if (t.includes('wip')) {
    defaultIcon = 'filter';
    desc = desc || 'Nombre maximal de cartes simultanées autorisées dans cette liste. Laissez vide pour aucune restriction.';
    placeholder = placeholder || 'Ex. : 4';
    isNumber = true;
  } else if (t.includes('board') || t.includes('tableau')) {
    defaultIcon = 'boards';
    desc = desc || 'Donnez un nom clair pour structurer votre espace de travail.';
    placeholder = placeholder || 'Ex. : Lancement Q1 2027';
  } else if (t.includes('liste')) {
    defaultIcon = 'kanban';
    desc = desc || 'Nom de la colonne dans votre tableau Kanban.';
    placeholder = placeholder || 'Ex. : En cours, Revue, Bloqué';
  } else if (t.includes('checklist')) {
    defaultIcon = 'check';
    desc = desc || 'Sous-tâches ou points de contrôle à valider.';
    placeholder = placeholder || 'Ex. : Critères d’acceptation';
  } else if (t.includes('élément') || t.includes('element')) {
    defaultIcon = 'plus';
    desc = desc || 'Ajoutez une étape à la checklist.';
    placeholder = placeholder || 'Ex. : Valider la maquette Figma';
  } else if (t.includes('prédécesseur') || t.includes('dependance') || t.includes('dépendance')) {
    defaultIcon = 'timeline';
    desc = desc || 'Tâche requise avant de pouvoir démarrer celle-ci.';
    placeholder = placeholder || 'Titre ou identifiant de la carte';
  } else if (t.includes('baseline')) {
    defaultIcon = 'dashboard';
    desc = desc || 'Instantané de référence pour mesurer l’avancement et les écarts de planning.';
  } else if (t.includes('champ')) {
    defaultIcon = 'settings';
    desc = desc || 'Nom du champ personnalisé.';
  } else if (t.includes('label')) {
    defaultIcon = 'filter';
    desc = desc || 'Nom du label de catégorisation.';
  } else if (t.includes('calendrier')) {
    defaultIcon = 'calendar';
    desc = desc || 'Calendrier de jours ouvrés pour le planning CPM.';
  }

  const iconName = opts.icon || defaultIcon;

  return new Promise((resolve) => {
    const { el, close } = openModal(`
      <div class="dialog-header">
        <div class="dialog-badge">${icon(iconName, { size: 18 })}</div>
        <div class="dialog-titles">
          <h3 class="dialog-title">${escapeHtml(title)}</h3>
          ${desc ? `<p class="dialog-desc">${escapeHtml(desc)}</p>` : ''}
        </div>
        <button class="iconbtn dialog-close" data-close title="Fermer (Échap)">${icon('close', { size: 14 })}</button>
      </div>
      <div class="dialog-body">
        <input class="input dialog-input" ${isNumber ? 'type="number" min="0"' : 'type="text"'} value="${escapeHtml(value)}" placeholder="${escapeHtml(placeholder)}" />
      </div>
      <div class="dialog-footer">
        <div class="dialog-hints">
          <span class="kbd">↵ Entrée</span> pour valider · <span class="kbd">Échap</span>
        </div>
        <div class="dialog-btns">
          <button class="btn" data-act="no">Annuler</button>
          <button class="btn primary" data-act="yes">${escapeHtml(submitLabel)}</button>
        </div>
      </div>`, { size: 'sm' });

    const input = el.querySelector('input');
    input.focus();
    input.select();
    const submit = () => { const v = input.value.trim(); close(); resolve(v || null); };
    el.querySelector('[data-act=yes]').onclick = submit;
    input.addEventListener('keydown', (e) => { if (e.key === 'Enter') submit(); });
    el.querySelector('[data-act=no]').onclick = () => { close(); resolve(null); };
    const prev = el.closest('.modal-backdrop');
    const obs = new MutationObserver(() => {
      if (!prev.isConnected) { resolve(null); obs.disconnect(); }
    });
    obs.observe(document.getElementById('overlays'), { childList: true });
  });
}
