// Vue Calendrier : grille mensuelle, drag pour replanifier (due), clic = modale.

import { api } from '../api.js';
import { escapeHtml, toISODate, fmtDate, parseDate, fmtDateShort } from '../format.js';
import { state, refreshBoard, labelById } from '../state.js';
import { draggable } from '../dnd.js';
import { toast } from '../ui/modal.js';
import { openCardModal } from '../components/card_modal.js';
import { saveViewSettings } from './view_helpers.js';
import { icon } from '../icons.js';

const DOW = ['lun', 'mar', 'mer', 'jeu', 'ven', 'sam', 'dim'];

let cursor = new Date();

export function renderCalendar() {
  const saved = state.settings.calendar || {};
  if (saved.cursor) cursor = new Date(saved.cursor);

  const host = document.querySelector('.view');
  const y = cursor.getFullYear();
  const m = cursor.getMonth();
  const first = new Date(y, m, 1);
  const startOffset = (first.getDay() + 6) % 7; // lundi = 0
  const start = new Date(y, m, 1 - startOffset);
  const days = [];
  for (let i = 0; i < 42; i++) {
    const d = new Date(start.getFullYear(), start.getMonth(), start.getDate() + i);
    days.push(d);
  }
  const todayISO = toISODate(new Date());
  const monthName = cursor.toLocaleDateString('fr-FR', { month: 'long', year: 'numeric' });

  // cartes par jour (due)
  const withDue = state.cards.filter((c) => c.due && !c.archived && !c.is_template);
  const byDay = new Map();
  for (const c of withDue) {
    const iso = toISODate(parseDate(c.due));
    if (!byDay.has(iso)) byDay.set(iso, []);
    byDay.get(iso).push(c);
  }

  host.innerHTML = `
    <div class="cal-head">
      <button class="iconbtn" id="cal-prev" title="Mois précédent">${icon('chevronLeft', { size: 16 })}</button>
      <h2 style="text-transform:capitalize">${monthName}</h2>
      <button class="iconbtn" id="cal-next" title="Mois suivant">${icon('chevronRight', { size: 16 })}</button>
      <button class="btn sm" id="cal-today">Aujourd'hui</button>
      <div class="spacer" style="flex:1"></div>
      <span class="muted" style="font-size:12px">Glissez une carte vers un autre jour pour replanifier l'échéance</span>
    </div>
    <div class="cal-grid">
      ${DOW.map((d) => `<div class="cal-dow">${d}</div>`).join('')}
      ${days.map((d) => {
        const iso = toISODate(d);
        const inMonth = d.getMonth() === m;
        const cards = byDay.get(iso) || [];
        return `
          <div class="cal-cell ${inMonth ? '' : 'other'} ${iso === todayISO ? 'today' : ''}" data-day="${iso}">
            <div class="daynum">${d.getDate()}</div>
            ${cards.map((c) => {
              const labelColor = (c._label_ids || []).length
                ? labelById(c._label_ids[0])?.color : null;
              return `<button class="cal-card" data-card="${c.id}" title="${escapeHtml(c.title)}">
                ${labelColor ? `<span class="lbl" style="background:${labelColor}"></span>` : ''}
                ${c.due_complete ? `<span style="color:var(--success);margin-right:3px">✓</span>` : ''}
                <span>${escapeHtml(c.title)}</span>
              </button>`;
            }).join('')}
          </div>`;
      }).join('')}
    </div>`;

  host.querySelector('#cal-prev').onclick = () => shift(-1);
  host.querySelector('#cal-next').onclick = () => shift(1);
  host.querySelector('#cal-today').onclick = () => { cursor = new Date(); persist(); renderCalendar(); };

  // Une seule passe sur les .cal-card : clic = modale, drag = replanifier.
  host.querySelectorAll('.cal-card').forEach((b) => {
    b.addEventListener('click', () => openCardModal(b.dataset.card, { onChange: rerender }));
    const card = state.cards.find((c) => c.id === b.dataset.card);
    draggable(b, {
      kind: 'card',
      id: card.id,
      targetSelector: '.cal-cell',
      canDrag: (ev) => !card.due_complete,
      onDrop: async ({ targetEl }) => {
        const day = targetEl.dataset.day;
        if (!day || day === toISODate(parseDate(card.due))) return;
        const old = parseDate(card.due);
        let due = day;
        const hasTime = !/^\d{4}-\d{2}-\d{2}$/.test(String(card.due));
        if (hasTime && old) {
          const t = `${String(old.getHours()).padStart(2, '0')}:${String(old.getMinutes()).padStart(2, '0')}`;
          due = `${day}T${t}:00`;
        }
        try {
          await api.updateCard(card.id, { due });
          toast(`Replanifié : ${fmtDate(day)}`, 'ok');
          rerender();
        } catch (e) { toast(e.message, 'err'); }
      },
    });
  });
}

function shift(months) {
  cursor = new Date(cursor.getFullYear(), cursor.getMonth() + months, 1);
  persist();
  renderCalendar();
}

function persist() {
  saveViewSettings('calendar', { cursor: cursor.toISOString() });
}

async function rerender() {
  await refreshBoard();
  renderCalendar();
}
