// Drag & drop tactile et fluide (Pointer Events) pour listes et cartes.

let active = null; // { type, id, ghost }

function makeGhost(el, x, y) {
  const ghost = el.cloneNode(true);
  ghost.classList.add('drag-ghost');
  ghost.style.width = `${el.offsetWidth}px`;
  ghost.style.left = `${x - 14}px`;
  ghost.style.top = `${y - 14}px`;
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
    const start = { x: ev.clientX, y: ev.clientY };
    let moved = false;
    let ghost = null;
    let currentTarget = null;

    const onMove = (e) => {
      if (!moved && Math.hypot(e.clientX - start.x, e.clientY - start.y) < 5) return;
      if (!moved) {
        moved = true;
        ghost = makeGhost(el, e.clientX, e.clientY);
        el.classList.add('dragging');
        active = { type: opts.kind, id: opts.id };
      }
      ghost.style.left = `${e.clientX - 16}px`;
      ghost.style.top = `${e.clientY - 16}px`;

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
      }
    };

    const onUp = (e) => {
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerup', onUp);
      document.querySelector('.drop-line')?.remove();

      if (!moved) return; // simple clic
      ghost?.remove();
      el.classList.remove('dragging');
      active = null;

      if (currentTarget) {
        currentTarget.classList.remove('drop-over', 'drag-over');
        opts.onDrop({ x: e.clientX, y: e.clientY, targetEl: currentTarget, ev });
      }
    };

    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onUp);
  });
}

export function activeDrag() {
  return active;
}
