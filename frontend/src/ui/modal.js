// Toasts + modales génériques.

import { escapeHtml } from '../format.js';

export function toast(message, kind = '') {
  let host = document.querySelector('.toasts');
  if (!host) {
    host = document.createElement('div');
    host.className = 'toasts';
    document.getElementById('overlays').appendChild(host);
  }
  const el = document.createElement('div');
  el.className = `toast ${kind}`;
  el.textContent = message;
  host.appendChild(el);
  setTimeout(() => {
    el.style.opacity = '0';
    el.style.transition = 'opacity .3s';
    setTimeout(() => el.remove(), 320);
  }, 3400);
}

export function openModal(html, { onClose, id } = {}) {
  // Une modale avec le même id remplace l'existante (évite l'empilement).
  if (id) {
    document.querySelectorAll(`.modal-backdrop[data-modal-id="${id}"]`).forEach((b) => b.remove());
  }
  const backdrop = document.createElement('div');
  backdrop.className = 'modal-backdrop';
  if (id) backdrop.dataset.modalId = id;
  backdrop.innerHTML = `<div class="modal fade-in">${html}</div>`;
  document.getElementById('overlays').appendChild(backdrop);
  const close = () => {
    backdrop.remove();
    onClose?.();
  };
  backdrop.addEventListener('pointerdown', (e) => {
    if (e.target === backdrop) close();
  });
  backdrop.querySelectorAll('[data-close]').forEach((b) => b.addEventListener('click', close));
  const esc = (e) => { if (e.key === 'Escape') { close(); document.removeEventListener('keydown', esc); } };
  document.addEventListener('keydown', esc);
  const modalEl = backdrop.querySelector('.modal');
  return { el: modalEl, close };
}

export function confirmDialog(message) {
  return new Promise((resolve) => {
    const { el, close } = openModal(`
      <div class="modal-head"><h2>${escapeHtml(message)}</h2></div>
      <div class="modal-body row" style="justify-content:flex-end">
        <button class="btn" data-act="no">Annuler</button>
        <button class="btn primary" data-act="yes">Confirmer</button>
      </div>`);
    el.querySelector('[data-act=no]').onclick = () => { close(); resolve(false); };
    el.querySelector('[data-act=yes]').onclick = () => { close(); resolve(true); };
    const prev = el.closest('.modal-backdrop');
    const obs = new MutationObserver(() => {
      if (!prev.isConnected) { resolve(false); obs.disconnect(); }
    });
    obs.observe(document.getElementById('overlays'), { childList: true });
  });
}

export function promptDialog(title, value = '', placeholder = '') {
  return new Promise((resolve) => {
    const { el, close } = openModal(`
      <div class="modal-head"><h2>${escapeHtml(title)}</h2></div>
      <div class="modal-body">
        <input class="input" value="${escapeHtml(value)}" placeholder="${escapeHtml(placeholder)}" />
        <div class="row" style="justify-content:flex-end;margin-top:14px">
          <button class="btn" data-act="no">Annuler</button>
          <button class="btn primary" data-act="yes">OK</button>
        </div>
      </div>`);
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
