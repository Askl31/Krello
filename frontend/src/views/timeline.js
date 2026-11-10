// Vue Timeline (Gantt) : barres CPM, dépendances, chemin critique, jalons, zoom, drag.

import { api } from '../api.js';
import { parseDate, toISODate, fmtDateShort, fmtDate, escapeHtml } from '../format.js';
import { state, visibleLists, effectiveDates, refreshBoard } from '../state.js';
import { toast } from '../ui/modal.js';
import { openCardModal } from '../components/card_modal.js';
import { saveViewSettings } from './view_helpers.js';

const ZOOMS = { day: 26, week: 9, month: 3.2 }; // px par jour

let zoom = 'week';

export function renderTimeline() {
  const saved = state.settings.timeline || {};
  if (saved.zoom) zoom = saved.zoom;
  const host = document.querySelector('.view');
  const pxPerDay = ZOOMS[zoom];

  // ensemble de cartes avec dates
  const rows = [];
  for (const list of visibleLists()) {
    for (const card of state.cards.filter((c) => c.list_id === list.id && !c.archived)
      .sort((a, b) => a.position - b.position)) {
      const dates = effectiveDates(card);
      if (dates.start || dates.end) rows.push({ card, dates });
    }
  }
  if (!rows.length) {
    host.innerHTML = `
      <div class="empty-state">
        <div class="big">🗓</div>
        <p>Aucune carte datée. Ajoutez des échéances, des durées ou des dépendances<br/>
        (ou lancez une planification IA depuis ✨) pour alimenter le Gantt.</p>
      </div>`;
    return;
  }

  const today = new Date();
  let min = null;
  let max = null;
  for (const r of rows) {
    const s = parseDate(r.dates.start) || parseDate(r.dates.end);
    const e = parseDate(r.dates.end) || parseDate(r.dates.start);
    if (s && (!min || s < min)) min = s;
    if (e && (!max || e > max)) max = e;
  }
  if (min < today && today < max) { /* bornes OK */ }
  min = new Date(min); min.setDate(min.getDate() - 2);
  max = new Date(max); max.setDate(max.getDate() + 3);
  const totalDays = Math.max(1, Math.round((max - min) / 86400000));
  const trackW = totalDays * pxPerDay;

  const xOf = (date) => Math.round((date - min) / 86400000) * pxPerDay;

  // en-tête : ticks (semaines ou jours selon zoom)
  const ticks = [];
  if (zoom === 'day') {
    for (let i = 0; i <= totalDays; i++) {
      const d = new Date(min.getFullYear(), min.getMonth(), min.getDate() + i);
      if (d.getDay() === 1) ticks.push({ x: i * pxPerDay, label: fmtDateShort(d) });
    }
  } else if (zoom === 'week') {
    for (let i = 0; i <= totalDays; i++) {
      const d = new Date(min.getFullYear(), min.getMonth(), min.getDate() + i);
      if (d.getDay() === 1) ticks.push({ x: i * pxPerDay, label: `S${weekNo(d)} · ${fmtDateShort(d)}` });
    }
  } else {
    for (let i = 0; i <= totalDays; i++) {
      const d = new Date(min.getFullYear(), min.getMonth(), min.getDate() + i);
      if (d.getDate() === 1) ticks.push({ x: i * pxPerDay, label: d.toLocaleDateString('fr-FR', { month: 'short', year: '2-digit' }) });
    }
  }

  const rowsHtml = rows.map((r, idx) => {
    const s = parseDate(r.dates.start);
    const e = parseDate(r.dates.end);
    const x1 = xOf(s);
    const x2 = xOf(e) + pxPerDay;
    const width = Math.max(pxPerDay * 0.6, x2 - x1);
    const top = idx * 34;
    if (r.card.is_milestone) {
      return `
        <div class="gantt-row" data-row="${r.card.id}">
          <div class="rlabel" title="${escapeHtml(r.card.title)}">◆ ${escapeHtml(r.card.title)}<span class="lbl-chip">${listName(r.card.list_id)}</span></div>
          <div class="rtrack" style="width:${trackW}px">
            <div class="gantt-milestone ${r.dates.critical ? 'critical' : ''}" data-bar="${r.card.id}"
                 style="left:${x1}px; ${r.dates.critical ? 'background:var(--critical)' : ''}"></div>
            <span class="gantt-bar" style="left:${x1}px;width:0" data-bar="${r.card.id}"></span>
            <span class="bar-label-inline muted" style="position:absolute;left:${x1 + 16}px;top:9px;font-size:10.5px">${fmtDate(r.dates.start)}</span>
          </div>
        </div>`;
    }
    return `
      <div class="gantt-row" data-row="${r.card.id}">
        <div class="rlabel" title="${escapeHtml(r.card.title)}">${escapeHtml(r.card.title)}<span class="lbl-chip">${listName(r.card.list_id)}</span></div>
        <div class="rtrack" style="width:${trackW}px">
          <div class="gantt-bar ${r.dates.critical ? 'critical' : ''}" data-bar="${r.card.id}"
               style="left:${x1}px;width:${width}px" data-start="${toISODate(s)}" data-end="${toISODate(e)}">
            <div class="progress" style="width:${r.card.pct_complete || 0}%"></div>
            <span class="bar-label">${fmtDateShort(r.dates.start)} → ${fmtDateShort(r.dates.end)}${r.dates.float !== null && !r.dates.critical ? ` · marge ${Math.round(r.dates.float)}j` : ''}</span>
          </div>
        </div>
      </div>`;
  }).join('');

  // flèches de dépendances (SVG overlay)
  const depArrows = [];
  for (const d of state.deps) {
    const fromIdx = rows.findIndex((r) => r.card.id === d.predecessor_id);
    const toIdx = rows.findIndex((r) => r.card.id === d.successor_id);
    if (fromIdx === -1 || toIdx === -1) continue;
    const from = rows[fromIdx];
    const to = rows[toIdx];
    const sx = xOf(parseDate(from.dates.end)) + pxPerDay;
    const sy = fromIdx * 34 + 17;
    const ex = xOf(parseDate(to.dates.start));
    const ey = toIdx * 34 + 17;
    const mid = Math.max(sx + 10, ex - 10);
    depArrows.push(`<path d="M ${sx} ${sy} C ${mid} ${sy}, ${mid} ${ey}, ${ex - 5} ${ey}"
      fill="none" stroke="${from.dates.critical && to.dates.critical ? 'var(--critical)' : 'var(--muted)'}"
      stroke-width="1.4" marker-end="url(#arrowhead)" opacity="0.85" />`);
  }

  const todayX = xOf(today);

  host.innerHTML = `
    <div class="cal-head">
      <h2 style="min-width:auto">Timeline</h2>
      <div class="viewtabs" id="gantt-zoom">
        ${Object.keys(ZOOMS).map((z) => `<button data-zoom="${z}" class="${zoom === z ? 'active' : ''}">${z === 'day' ? 'Jour' : z === 'week' ? 'Semaine' : 'Mois'}</button>`).join('')}
      </div>
      <div class="spacer" style="flex:1"></div>
      <span class="muted" style="font-size:12px">${rows.length} éléments datés · base : ordonnancement CPM</span>
    </div>
    <div class="gantt-legend">
      <span><span class="sw" style="background:var(--c1)"></span>Tâche</span>
      <span><span class="sw" style="background:var(--critical)"></span>Chemin critique</span>
      <span><span class="sw" style="background:var(--c3)"></span>Jalon</span>
      <span><span class="sw" style="background:var(--accent-2)"></span>Aujourd'hui</span>
    </div>
    <div class="gantt">
      <div class="gantt-inner" style="width:${trackW + 210}px">
        <div class="gantt-header gantt-row" style="height:26px">
          <div class="rlabel"></div>
          <div class="rtrack gantt-scale" style="width:${trackW}px">
            ${ticks.map((t) => `<div class="tick" style="left:${t.x}px;width:${pxPerDay * 6}px">${t.label}</div>`).join('')}
          </div>
        </div>
        <div style="position:relative">
          ${todayX >= 0 && todayX <= trackW ? `<div class="gantt-today" style="left:${todayX + 210}px"></div>` : ''}
          <svg class="gantt-deps" width="${trackW + 210}" height="${rows.length * 34}" style="left:0;top:0">
            <defs>
              <marker id="arrowhead" markerWidth="7" markerHeight="7" refX="6" refY="3.5" orient="auto">
                <polygon points="0 0, 7 3.5, 0 7" fill="var(--muted)" />
              </marker>
            </defs>
            ${depArrows.join('')}
          </svg>
          ${rowsHtml}
        </div>
      </div>
    </div>`;

  host.querySelector('#gantt-zoom').addEventListener('click', (e) => {
    zoom = e.target.closest('[data-zoom]')?.dataset.zoom || zoom;
    saveViewSettings('timeline', { zoom });
    renderTimeline();
  });

  // interactions barres : clic → modale ; drag → décale les dates (due/start décalés du delta)
  host.querySelectorAll('[data-bar]').forEach((bar) => {
    const cardId = bar.dataset.bar;
    bar.addEventListener('click', (e) => {
      if (e.target !== bar) return;
      openCardModal(cardId, { onChange: rerender });
    });
    dragBar(bar, cardId, pxPerDay);
  });
}

function dragBar(bar, cardId, pxPerDay) {
  bar.addEventListener('pointerdown', (ev) => {
    if (ev.button !== 0) return;
    ev.preventDefault();
    const startX = ev.clientX;
    const card = state.cards.find((c) => c.id === cardId);
    let moved = false;
    const ghost = document.createElement('div');
    const onMove = (e) => {
      const deltaDays = Math.round((e.clientX - startX) / pxPerDay);
      if (!moved && deltaDays === 0) return;
      moved = true;
      bar.style.opacity = '.75';
      bar.style.transform = `translateX(${deltaDays * pxPerDay}px)`;
    };
    const onUp = async (e) => {
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerup', onUp);
      if (!moved) return;
      const deltaDays = Math.round((e.clientX - startX) / pxPerDay);
      bar.style.opacity = '';
      bar.style.transform = '';
      if (deltaDays === 0) return;
      const shift = (s) => {
        if (!s) return null;
        const d = parseDate(s);
        d.setDate(d.getDate() + deltaDays);
        return toISODate(d);
      };
      try {
        const patch = {};
        if (card.due) patch.due = shift(card.due);
        if (card.start) patch.start = shift(card.start);
        await api.updateCard(cardId, patch);
        toast(`Décalé de ${deltaDays > 0 ? '+' : ''}${deltaDays} j`, 'ok');
        rerender();
      } catch (err) { toast(err.message, 'err'); }
    };
    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onUp);
  });
}

function weekNo(d) {
  const target = new Date(d.valueOf());
  const dayNr = (d.getDay() + 6) % 7;
  target.setDate(target.getDate() - dayNr + 3);
  const firstThursday = new Date(target.getFullYear(), 0, 4);
  const diff = target - firstThursday;
  return 1 + Math.round(diff / (7 * 86400000));
}

function listName(listId) {
  return state.lists.find((l) => l.id === listId)?.title || '';
}

async function rerender() {
  await refreshBoard();
  renderTimeline();
}
