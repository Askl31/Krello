// Drag & drop tactile et fluide (Pointer Events) pour listes et cartes.

let active = null; // { type, id, ghost }

function makeGhost(el, x, y, grabOffset) {
  const ghost = el.cloneNode(true);
  ghost.classList.add('drag-ghost');
  ghost.style.width = `${el.offsetWidth}px`;
  const ox = grabOffset ? grabOffset.x : 14;
  const oy = grabOffset ? grabOffset.y : 14;
  ghost.style.translate = `${x - ox}px ${y - oy}px`;
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

/// Indicateur de drop pour les listes : marque la colonne cible avec
/// `drop-before`/`drop-after` (box-shadow inset → ZÉRO décalage de layout).
/// Répartit l'espace continu horizontalement entre les colonnes restantes :
/// zéro trou dans les gaps, zéro échec en dessous des colonnes courtes.
function updateListDropIndicator(sourceEl, clientX) {
  const boardEl = document.querySelector('#board');
  if (!boardEl) return;
  const allLists = [...boardEl.querySelectorAll('.list')];
  allLists.forEach((l) => l.classList.remove('drop-before', 'drop-after'));
  const others = allLists.filter((l) => l !== sourceEl);
  if (!others.length) return;

  let insertIdx = others.length;
  for (let i = 0; i < others.length; i++) {
    const box = others[i].getBoundingClientRect();
    const midX = box.left + box.width / 2;
    if (clientX < midX) {
      insertIdx = i;
      break;
    }
  }

  if (insertIdx < others.length) {
    others[insertIdx].classList.add('drop-before');
  } else {
    others[others.length - 1].classList.add('drop-after');
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
  const elRect = el.getBoundingClientRect();
  const grabOffset = {
    x: ev.clientX - elRect.left,
    y: ev.clientY - elRect.top,
  };
  let moved = false;
  let ghost = null;
  let currentTarget = null;
  let raf = 0;
  let lastEvt = null;

  const onSelectStart = (e) => e.preventDefault();
  window.addEventListener('selectstart', onSelectStart, { capture: true });

  const cleanup = () => {
    if (raf) cancelAnimationFrame(raf);
    raf = 0;
    document.body.classList.remove('is-dragging');
    window.removeEventListener('selectstart', onSelectStart, { capture: true });
    window.getSelection()?.removeAllRanges();
    window.removeEventListener('pointermove', onMove);
    window.removeEventListener('pointerup', onUp);
    window.removeEventListener('pointercancel', onCancel);
    window.removeEventListener('keydown', onEsc);
    document.querySelector('.drop-line')?.remove();
    document.querySelectorAll('#board .drop-before, #board .drop-after')
      .forEach((l) => l.classList.remove('drop-before', 'drop-after'));
    document.querySelectorAll('.drop-over, .drag-over')
      .forEach((l) => l.classList.remove('drop-over', 'drag-over'));
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
      document.body.classList.add('is-dragging');
      window.getSelection()?.removeAllRanges();
      ghost = makeGhost(el, e.clientX, e.clientY, grabOffset);
      el.classList.add('dragging');
      active = { type: opts.kind, id: opts.id };
    }
    e.preventDefault?.();
    window.getSelection()?.removeAllRanges();
    const ox = grabOffset.x;
    const oy = grabOffset.y;
    ghost.style.translate = `${e.clientX - ox}px ${e.clientY - oy}px`;

    // Défilement automatique horizontal si on s'approche des bords du scroller
    const scroller = document.querySelector('.board-scroller');
    if (scroller) {
      const edgeMargin = 70;
      const sr = scroller.getBoundingClientRect();
      if (e.clientX > sr.right - edgeMargin) {
        const intensity = Math.min(1, (e.clientX - (sr.right - edgeMargin)) / edgeMargin);
        scroller.scrollLeft += Math.round(intensity * 18);
      } else if (e.clientX < sr.left + edgeMargin) {
        const intensity = Math.min(1, ((sr.left + edgeMargin) - e.clientX) / edgeMargin);
        scroller.scrollLeft -= Math.round(intensity * 18);
      }
    }

    let found = null;
    if (opts.kind === 'list') {
      const boardEl = document.querySelector('#board');
      if (boardEl) {
        found = boardEl;
        updateListDropIndicator(el, e.clientX);
      }
    } else if (opts.kind === 'card' && opts.targetSelector === '.list') {
      // Pour les cartes dans le Kanban : projection continue sur les colonnes
      const scrollerEl = document.querySelector('.board-scroller');
      const lists = [...document.querySelectorAll('#board .list')];
      if (lists.length) {
        const topBound = lists[0].getBoundingClientRect().top - 30;
        const bottomBound = scrollerEl ? scrollerEl.getBoundingClientRect().bottom + 40 : window.innerHeight;
        if (e.clientY >= topBound && e.clientY <= bottomBound) {
          let best = null;
          let bestDist = Infinity;
          for (const l of lists) {
            const r = l.getBoundingClientRect();
            let dist = 0;
            if (e.clientX < r.left) dist = r.left - e.clientX;
            else if (e.clientX > r.right) dist = e.clientX - r.right;
            if (dist < bestDist) {
              bestDist = dist;
              best = l;
            }
          }
          found = best;
        }
      }
      updateDropIndicator(found, e.clientY, el);
    } else {
      const zones = document.querySelectorAll(opts.targetSelector);
      for (const z of zones) {
        const r = z.getBoundingClientRect();
        if (e.clientX >= r.left && e.clientX <= r.right && e.clientY >= r.top && e.clientY <= r.bottom) {
          found = z;
          break;
        }
      }
      if (opts.kind === 'card') {
        updateDropIndicator(found, e.clientY, el);
      }
    }

    if (currentTarget && currentTarget !== found) {
      currentTarget.classList.remove('drop-over', 'drag-over');
    }
    currentTarget = found;
    if (found && opts.kind !== 'list') {
      found.classList.add(opts.kind === 'card' ? 'drag-over' : 'drop-over');
    }
  };
  // Coalesce les pointermove (peuvent dépasser 120 Hz) en un traitement par frame.
  const onMove = (e) => {
    lastEvt = e;
    if (moved) e.preventDefault?.();
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
