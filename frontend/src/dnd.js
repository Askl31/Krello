// Drag & drop générique (Pointer Events) pour listes et cartes.

let active = null; // { type, id, ghost, offsetX, offsetY }

function makeGhost(el, x, y) {
  const ghost = el.cloneNode(true);
  ghost.classList.add('drag-ghost');
  ghost.style.left = `${x + 8}px`;
  ghost.style.top = `${y + 8}px`;
  document.body.appendChild(ghost);
  return ghost;
}

/**
 * Rend un élément draggable.
 * opts:
 *   kind: 'card' | 'list'
 *   id: identifiant
 *   canDrag(): bool
 *   onDrop(payload): callback — payload = { x, y, targetEl }
 *   targetSelector: sélecteur CSS des zones droppables (listes / cellules calendrier)
 *   payload(el): données de la zone survolée
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
      ghost.style.left = `${e.clientX + 8}px`;
      ghost.style.top = `${e.clientY + 8}px`;
      const zones = document.querySelectorAll(opts.targetSelector);
      let found = null;
      for (const z of zones) {
        const r = z.getBoundingClientRect();
        if (e.clientX >= r.left && e.clientX <= r.right && e.clientY >= r.top && e.clientY <= r.bottom) {
          found = z;
          break;
        }
      }
      if (currentTarget && currentTarget !== found) currentTarget.classList.remove('drop-over', 'drag-over');
      currentTarget = found;
      if (found) found.classList.add(opts.kind === 'card' ? 'drag-over' : 'drop-over');
    };

    const onUp = (e) => {
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerup', onUp);
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
