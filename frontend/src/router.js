// Routeur par hash : #/ → accueil ; #/b/{board}/{view}

import { emit } from './ws.js';

function parse() {
  const h = location.hash.replace(/^#\/?/, '');
  const parts = h.split('/').filter(Boolean);
  if (parts[0] === 'b' && parts[1]) {
    return {
      name: 'board',
      boardId: parts[1],
      view: parts[2] || 'board',
      // Lien profond vers une carte: #/b/{board}/{view}/c/{cardId}
      cardId: parts[3] === 'c' ? parts[4] || null : null,
    };
  }
  if (parts[0] === 'settings') return { name: 'settings' };
  return { name: 'home' };
}

export function navigate(hash) {
  if (location.hash === hash) emit('route');
  else location.hash = hash;
}

export function boardUrl(boardId, view) {
  return `#/b/${boardId}/${view}`;
}

export function cardUrl(boardId, cardId) {
  return `#/b/${boardId}/board/c/${cardId}`;
}

export function startRouter(onChange) {
  window.addEventListener('hashchange', () => onChange(parse()));
  onChange(parse());
}

export { parse as currentRoute };
