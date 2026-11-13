// Drag & drop tactile et fluide (Pointer Events) pour listes et cartes.

let active = null; // { type, id, ghost }

function makeGhost(el, x, y) {
  const ghost = el.cloneNode(true);
  ghost.classList.add('drag-ghost');
  ghost.style.width = `${el.offsetWidth}px`;
  ghost.style.translate = `${x - 14}px ${y - 14}px`;
  document.body.appendChild(ghost);
  return ghost;
}

function updateDropIndicator(targetEl, clientY, sourceEl) {
  const listEl = targetEl?.closest?.('.list');
  if (!listEl) {
    document.querySelector('.drop-line')?.remove();
    return;
  }
  const body = listEl.querySelector('.list-body');
  if (!body) return;

  let dropLine = document.querySelector('.drop-line');
  if (!dropLine) {
    dropLine = document.createElement('div');
    dropLine.className = 'drop-line';
  }

  const tiles = [...body.querySelectorAll('.card-tile')].filter((t) => t !== sourceEl);
  const nextTile = tiles.find((t) => {
    const box = t.getBoundingClientRect();
    return clientY < box.top + box.height / 2;
  });

  if (nextTile) {
    body.insertBefore(dropLine, nextTile);
  } else {
    body.appendChild(dropLine);
  }
}

/// Indicateur de drop VERTICAL pour les listes (colonne cible dans le board) :
/// s'insère avant la liste sous le curseur, sinon après la dernière (avant le
/// bouton « Ajouter une liste »).
function updateListDropIndicator(targetEl, clientX, sourceEl) {
  const boardEl = document.querySelector('#board');
  if (!boardEl) return;
  let dropLine = document.querySelector('.drop-line');
  if (!dropLine) {
    dropLine = document.createElement('div');
    dropLine.className = 'drop-line vertical';
  }
  const lists = [...boardEl.querySelectorAll('.list')].filter((l) => l !== sourceEl);
  const next = lists.find((l) => {
    const box = l.getBoundingClientRect();
    return clientX < box.left + box.width / 2;
  });
  if (next) {
    boardEl.insertBefore(dropLine, next);
  } else {
    boardEl.insertBefore(dropLine, boardEl.querySelector('.list-add'));
  }
}

/**
 * Rend un élément draggable avec animations et indicateur d'insertion.
 * opts:
 *   kind: 'card' | 'list'
 *   id: identifiant
 *   canDrag(): bool
 *   onDrop(payload): callback — payload = { x, y, targetEl }
 *   targetSelector: sélecteur CSS des zones droppables
 */
export function draggable(el, opts) {
  el.addEventListener('pointerdown', (ev) => {
    if (ev.button !== 0) return;
    if (opts.canDrag && !opts.canDrag(ev)) return;
    beginDrag(el, opts, ev);
  });
}

/**
 * Start a drag session for `el`. Used by `draggable` (per element) and by the
 * board's delegated listeners, which resolve the tile from the event target.
 */
export function beginDrag(el, opts, ev) {
  const start = { x: ev.clientX, y: ev.clientY };
  let moved = false;
  let ghost = null;
  let currentTarget = null;
  let raf = 0;
  let lastEvt = null;

  const cleanup = () => {
    if (raf) cancelAnimationFrame(raf);
    raf = 0;
    window.removeEventListener('pointermove', onMove);
    window.removeEventListener('pointerup', onUp);
    window.removeEventListener('pointercancel', onCancel);
    window.removeEventListener('keydown', onEsc);
    document.querySelector('.drop-line')?.remove();
  };

  const endDrag = (e, cancelled) => {
    cleanup();
    if (!moved) return; // simple clic
    lastDragEnd = Date.now();
    ghost?.remove();
    el.classList.remove('dragging');
    active = null;

    if (!cancelled && currentTarget) {
      currentTarget.classList.remove('drop-over', 'drag-over');
      opts.onDrop({ x: e.clientX, y: e.clientY, targetEl: currentTarget, ev });
    }
  };

  const handleMove = (e) => {
    if (!moved && Math.hypot(e.clientX - start.x, e.clientY - start.y) < 5) return;
    if (!moved) {
      moved = true;
      ghost = makeGhost(el, e.clientX, e.clientY);
      el.classList.add('dragging');
      active = { type: opts.kind, id: opts.id };
    }
    ghost.style.translate = `${e.clientX - 16}px ${e.clientY - 16}px`;

    const zones = document.querySelectorAll(opts.targetSelector);
    let found = null;
    for (const z of zones) {
      const r = z.getBoundingClientRect();
      if (e.clientX >= r.left && e.clientX <= r.right && e.clientY >= r.top && e.clientY <= r.bottom) {
        found = z;
        break;
      }
    }

    if (currentTarget && currentTarget !== found) {
      currentTarget.classList.remove('drop-over', 'drag-over');
    }
    currentTarget = found;
    if (found) {
      found.classList.add(opts.kind === 'card' ? 'drag-over' : 'drop-over');
    }

    if (opts.kind === 'card') {
      updateDropIndicator(found, e.clientY, el);
    } else if (opts.kind === 'list') {
      updateListDropIndicator(found, e.clientX, el);
    }
  };
  // Coalesce les pointermove (peuvent dépasser 120 Hz) en un traitement par frame.
  const onMove = (e) => {
    lastEvt = e;
    if (raf) return;
    raf = requestAnimationFrame(() => {
      raf = 0;
      handleMove(lastEvt);
    });
  };

  const onUp = (e) => endDrag(e, false);
  const onCancel = () => endDrag({ clientX: start.x, clientY: start.y }, true);
  const onEsc = (e) => { if (e.key === 'Escape') onCancel(); };

  window.addEventListener('pointermove', onMove);
  window.addEventListener('pointerup', onUp);
  window.addEventListener('pointercancel', onCancel);
  window.addEventListener('keydown', onEsc);
}

// Dernière position Y du pointeur (utilisée pour calculer before/after au drop).
let lastY = 0;
window.addEventListener('pointermove', (e) => { lastY = e.clientY; });
export function lastPointerY() {
  return lastY;
}

// Horodatage du dernier drag RÉELLEMENT déplacé — les handlers `click` qui
// suivent un pointerup (navigation board, renommage de liste) s'en servent
// pour ignorer le clic résiduel d'un drag qui vient de se terminer.
let lastDragEnd = 0;
export function recentlyDragged(ms = 250) {
  return Date.now() - lastDragEnd < ms;
}

export function activeDrag() {
  return active;
}
