// Deep links `krello://` reçus par l'app native (Siri / Spotlight, macOS).
// En mode navigateur (ou Windows) `window.__TAURI__.deepLink` est absent :
// aucun écouteur, aucun changement de comportement.

import { api } from './api.js';
import { toast, promptDialog } from './ui/modal.js';

async function handleUrl(url) {
  let r;
  try {
    r = await api.deepLink(url);
  } catch (e) {
    toast(`Ajout rapide Krello — ${e.message}`, 'err');
    return;
  }
  if (r.kind === 'created') {
    toast(`✓ « ${r.card.title} » ajouté à « ${r.list.title} » (${r.board.title})`, 'ok');
    return;
  }
  if (r.kind === 'needs_input') {
    // Pas de titre (ou confirm=1) : modale pré-remplie, création via l'API
    // (l'événement WebSocket met la vue à jour immédiatement).
    const title = await promptDialog(
      `Nouvelle tâche dans « ${r.list.title} » (${r.board.title})`,
      r.title || '',
      'Titre de la tâche…',
      { submitLabel: 'Ajouter', icon: 'plus' },
    );
    if (!title) return;
    try {
      await api.createCard(r.board.id, { title, list_id: r.list.id });
      toast(`✓ « ${title} » ajouté à « ${r.list.title} »`, 'ok');
    } catch (e) {
      toast(e.message, 'err');
    }
  }
}

export function initDeepLink() {
  const dl = window.__TAURI__?.deepLink;
  if (!dl) return;
  const handle = (urls) => {
    (urls || []).forEach((u) => {
      handleUrl(u);
    });
  };
  // URL de lancement à froid (app ouverte par le raccourci alors qu'elle était fermée)
  Promise.resolve(dl.getCurrent?.())
    .then(handle)
    .catch(() => {});
  // URLs reçues app déjà ouverte
  Promise.resolve(dl.onOpenUrl?.(handle)).catch(() => {});
}
