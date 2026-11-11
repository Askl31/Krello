// Vue Timeline (Gantt) : barres CPM, dépendances, chemin critique, jalons, zoom, drag.

import { api } from '../api.js';
import { parseDate, toISODate, fmtDateShort, fmtDate, escapeHtml } from '../format.js';
import { state, visibleLists, effectiveDates, refreshBoard } from '../state.js';
import { toast } from '../ui/modal.js';
import { openCardModal } from '../components/card_modal.js';
import { saveViewSettings } from './view_helpers.js';
import { icon } from '../icons.js';

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
    for (const card of state.cards.filter((c) => c.list_id === list.id && !c.archived && !c.is_template)
      .sort((a, b) => a.position - b.position)) {
      const dates = effectiveDates(card);
      if (dates.start || dates.end) rows.push({ card, dates });
    }
  }
  if (!rows.length) {
    host.innerHTML = `
      <div class="empty-state">
        <div class="big">${icon('timeline', { size: 48, className: 'muted' })}</div>
        <h3 style="margin:12px 0 6px">Aucune tâche planifiée</h3>
        <p>Ajoutez des échéances, des durées ou des dépendances dans vos cartes<br/>
        (ou lancez une planification IA depuis ✨) pour alimenter le diagramme de Gantt.</p>
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
    if (r.card.is_milestone) {
      return `
        <div class="gantt-row" data-row="${r.card.id}">
          <div class="rlabel" title="${escapeHtml(r.card.title)}">
            <span style="color:var(--warning);margin-right:4px">◆</span>
            <span>${escapeHtml(r.card.title)}</span>
            <span class="lbl-chip">${listName(r.card.list_id)}</span>
          </div>
          <div class="rtrack" style="width:${trackW}px">
            <div class="gantt-milestone ${r.dates.critical ? 'critical' : ''}" data-bar="${r.card.id}"
                 style="left:${x1}px; ${r.dates.critical ? 'background:var(--critical)' : ''}"></div>
            <span class="gantt-bar" style="left:${x1}px;width:0" data-bar="${r.card.id}"></span>
            <span class="bar-label-inline muted" style="position:absolute;left:${x1 + 18}px;top:9px;font-size:11px">${fmtDate(r.dates.start)}</span>
          </div>
        </div>`;
    }
    return `
      <div class="gantt-row" data-row="${r.card.id}">
        <div class="rlabel" title="${escapeHtml(r.card.title)}">
          <span>${escapeHtml(r.card.title)}</span>
          <span class="lbl-chip">${listName(r.card.list_id)}</span>
        </div>
        <div class="rtrack" style="width:${trackW}px">
          <div class="gantt-bar ${r.dates.critical ? 'critical' : ''}" data-bar="${r.card.id}"
               style="left:${x1}px;width:${width}px" data-start="${toISODate(s)}" data-end="${toISODate(e)}">
            <div class="progress" style="width:${r.card.pct_complete || 0}%"></div>
            <span class="bar-label" style="position:absolute;left:calc(100% + 8px);top:1px;font-size:11px;color:var(--muted);white-space:nowrap">
              ${fmtDateShort(r.dates.start)} → ${fmtDateShort(r.dates.end)}${r.dates.float !== null && !r.dates.critical ? ` · marge ${Math.round(r.dates.float)}j` : ''}
            </span>
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
    const sy = fromIdx * 36 + 18;
    const ex = xOf(parseDate(to.dates.start));
    const ey = toIdx * 36 + 18;
    const mid = Math.max(sx + 10, ex - 10);
    depArrows.push(`<path d="M ${sx} ${sy} C ${mid} ${sy}, ${mid} ${ey}, ${ex - 5} ${ey}"
      fill="none" stroke="${from.dates.critical && to.dates.critical ? 'var(--critical)' : 'var(--muted)'}"
      stroke-width="1.6" marker-end="url(#arrowhead)" opacity="0.85" />`);
  }

  const todayX = xOf(today);

  host.innerHTML = `
    <div class="cal-head">
      <h2 style="min-width:auto">Timeline (Gantt CPM)</h2>
      <div class="viewtabs" id="gantt-zoom">
        ${Object.keys(ZOOMS).map((z) => `<button data-zoom="${z}" class="${zoom === z ? 'active' : ''}">${z === 'day' ? 'Jour' : z === 'week' ? 'Semaine' : 'Mois'}</button>`).join('')}
      </div>
      <button class="btn sm" id="gantt-export-svg" title="Exporter le diagramme en fichier vectoriel SVG">
        ${icon('attachment', { size: 12 })} <span>Exporter SVG</span>
      </button>
      <div class="spacer" style="flex:1"></div>
      <span class="muted" style="font-size:12px">${rows.length} tâche(s) · ordonnancement automatique CPM</span>
    </div>
    <div class="gantt-legend">
      <span><span class="sw" style="background:var(--c1)"></span>Tâche standard</span>
      <span><span class="sw" style="background:var(--critical)"></span>Chemin critique</span>
      <span><span class="sw" style="background:var(--c3)"></span>Jalon</span>
      <span><span class="sw" style="background:var(--accent-2)"></span>Aujourd'hui</span>
    </div>
    <div class="gantt">
      <div class="gantt-inner" style="width:${trackW + 220}px">
        <div class="gantt-header gantt-row" style="height:28px">
          <div class="rlabel" style="font-weight:600;font-size:11.5px;text-transform:uppercase;color:var(--muted)">Éléments</div>
          <div class="rtrack gantt-scale" style="width:${trackW}px">
            ${ticks.map((t) => `<div class="tick" style="left:${t.x}px;width:${pxPerDay * 6}px">${t.label}</div>`).join('')}
          </div>
        </div>
        <div style="position:relative">
          ${todayX >= 0 && todayX <= trackW ? `<div class="gantt-today" style="left:${todayX + 220}px"></div>` : ''}
          <svg class="gantt-deps" width="${trackW + 220}" height="${rows.length * 36}" style="left:0;top:0">
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

  host.querySelector('#gantt-export-svg')?.addEventListener('click', () => {
    downloadGanttSvg(state.board?.title || 'Krello', trackW, rows, ticks, depArrows, todayX, pxPerDay, xOf);
  });

  // interactions barres : clic → modale ; drag → décale les dates
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

function downloadGanttSvg(boardTitle, trackW, rows, ticks, depArrows, todayX, pxPerDay, xOf) {
  const totalW = trackW + 240;
  const totalH = rows.length * 36 + 70;
  const isDark = document.documentElement.dataset.theme !== 'light';
  const bg = isDark ? '#090d16' : '#ffffff';
  const fg = isDark ? '#f1f5f9' : '#0f172a';
  const muted = isDark ? '#64748b' : '#94a3b8';
  const border = isDark ? '#1e293b' : '#e2e8f0';

  let svg = `<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="${totalW}" height="${totalH}" viewBox="0 0 ${totalW} ${totalH}" style="font-family:-apple-system,BlinkMacSystemFont,Inter,sans-serif;background:${bg}">
  <defs>
    <marker id="arrow" markerWidth="6" markerHeight="6" refX="5" refY="3" orient="auto">
      <polygon points="0 0, 6 3, 0 6" fill="${muted}" />
    </marker>
  </defs>
  <rect width="100%" height="100%" fill="${bg}" />
  <text x="20" y="26" font-size="15" font-weight="700" fill="${fg}">${escapeHtml(boardTitle)} — Diagramme de Gantt CPM</text>
  <g transform="translate(0, 42)">
    <rect x="0" y="0" width="${totalW}" height="26" fill="${isDark ? '#131929' : '#f8fafc'}" />
    <text x="20" y="17" font-size="11" font-weight="600" fill="${muted}">TÂCHES</text>
`;

  for (const t of ticks) {
    svg += `<text x="${t.x + 230}" y="17" font-size="10" font-weight="500" fill="${muted}">${escapeHtml(t.label)}</text>`;
    svg += `<line x1="${t.x + 220}" y1="0" x2="${t.x + 220}" y2="${rows.length * 36 + 26}" stroke="${border}" stroke-width="1" stroke-dasharray="3 3" opacity="0.6" />`;
  }

  rows.forEach((r, idx) => {
    const y = idx * 36 + 26;
    const s = parseDate(r.dates.start);
    const e = parseDate(r.dates.end);
    const x1 = xOf(s) + 220;
    const x2 = xOf(e) + 220 + pxPerDay;
    const w = Math.max(pxPerDay * 0.6, x2 - x1);
    const isCrit = r.dates.critical;
    const barColor = isCrit ? '#ef4444' : '#6366f1';

    svg += `<line x1="0" y1="${y + 36}" x2="${totalW}" y2="${y + 36}" stroke="${border}" stroke-width="1" />`;
    svg += `<text x="20" y="${y + 22}" font-size="12" font-weight="500" fill="${fg}">${escapeHtml(r.card.title)}</text>`;

    if (r.card.is_milestone) {
      svg += `<rect x="${x1 - 8}" y="${y + 9}" width="16" height="16" transform="rotate(45 ${x1} ${y + 17})" fill="#f59e0b" rx="2" />`;
    } else {
      svg += `<rect x="${x1}" y="${y + 9}" width="${w}" height="18" rx="9" fill="${barColor}" />`;
      if (r.card.pct_complete) {
        svg += `<rect x="${x1}" y="${y + 9}" width="${(w * r.card.pct_complete) / 100}" height="18" rx="9" fill="rgba(255,255,255,0.35)" />`;
      }
    }
  });

  svg += `<g transform="translate(220, 26)">${depArrows.join('')}</g>`;

  if (todayX >= 0 && todayX <= trackW) {
    svg += `<line x1="${todayX + 220}" y1="26" x2="${todayX + 220}" y2="${rows.length * 36 + 26}" stroke="#06b6d4" stroke-width="2" />`;
  }

  svg += `</g></svg>`;

  const blob = new Blob([svg], { type: 'image/svg+xml;charset=utf-8' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = `krello-gantt-${(boardTitle || 'projet').toLowerCase().replace(/\s+/g, '-')}.svg`;
  a.click();
  URL.revokeObjectURL(url);
  toast('Export SVG téléchargé avec succès', 'ok');
}
