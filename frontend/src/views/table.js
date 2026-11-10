// Vue Table : feuille de calcul triable, édition inline.

import { api } from '../api.js';
import { escapeHtml, fmtDateShort, PRIORITY_META } from '../format.js';
import { state, visibleLists, effectiveDates, hydrateLabelIds } from '../state.js';
import { toast } from '../ui/modal.js';
import { openCardModal } from '../components/card_modal.js';

let sortKey = 'wbs';
let sortDir = 1;

export function renderTable() {
  const host = document.querySelector('.view');
  const lists = visibleLists();
  const listById = (id) => lists.find((l) => l.id === id);
  const labelNames = (c) => (c._label_ids || []).map((id) => state.labels.find((l) => l.id === id)?.name).filter(Boolean).join(', ');

  let cards = state.cards.filter((c) => !c.archived);
  const get = {
    wbs: (c) => c.wbs || '',
    title: (c) => c.title.toLowerCase(),
    list: (c) => listById(c.list_id)?.position ?? 0,
    priority: (c) => ['none', 'low', 'medium', 'high', 'urgent'].indexOf(c.priority),
    start: (c) => effectiveDates(c).start || '',
    due: (c) => effectiveDates(c).end || '',
    pct: (c) => c.pct_complete,
    duration: (c) => c.duration_days ?? '',
    work: (c) => c.work_hours ?? '',
    labels: (c) => labelNames(c),
    milestone: (c) => (c.is_milestone ? 0 : 1),
  };
  cards.sort((a, b) => {
    const va = get[sortKey]?.(a), vb = get[sortKey]?.(b);
    const cmp = va < vb ? -1 : va > vb ? 1 : 0;
    return cmp * sortDir;
  });

  const cols = [
    ['wbs', 'WBS'], ['title', 'Titre'], ['list', 'Liste'], ['labels', 'Labels'],
    ['priority', 'Priorité'], ['start', 'Début'], ['due', 'Échéance'],
    ['duration', 'Durée (j)'], ['work', 'Charge (h)'], ['pct', '%'], ['milestone', 'Type'],
  ];

  host.innerHTML = `
    <div class="tbl">
      <table class="grid">
        <thead><tr>
          ${cols.map(([k, label]) => `<th data-sort="${k}">${label}${sortKey === k ? (sortDir > 0 ? ' ▲' : ' ▼') : ''}</th>`).join('')}
        </tr></thead>
        <tbody>
          ${cards.map((c) => `
            <tr data-card="${c.id}" class="${c.due_complete ? 'done' : ''}">
              <td class="muted">${escapeHtml(c.wbs || '')}</td>
              <td>${c.is_milestone ? '◆ ' : ''}${escapeHtml(c.title)}</td>
              <td class="muted">${escapeHtml(listById(c.list_id)?.title || '')}</td>
              <td class="muted">${escapeHtml(labelNames(c))}</td>
              <td><span style="color:${PRIORITY_META[c.priority]?.color || 'inherit'}">${PRIORITY_META[c.priority]?.label || '—'}</span></td>
              <td class="muted">${fmtDateShort(effectiveDates(c).start)}</td>
              <td class="muted">${fmtDateShort(effectiveDates(c).end)}</td>
              <td><input class="cell-edit" data-num="duration_days" style="width:52px" value="${c.duration_days ?? ''}" /></td>
              <td><input class="cell-edit" data-num="work_hours" style="width:52px" value="${c.work_hours ?? ''}" /></td>
              <td><input class="cell-edit" data-num="pct_complete" style="width:44px" value="${c.pct_complete}" /></td>
              <td>${c.is_milestone ? '<span class="badge">jalon</span>' : (c.due_complete ? '<span class="badge done">✓ finie</span>' : '<span class="muted">tâche</span>')}</td>
            </tr>`).join('')}
        </tbody>
      </table>
    </div>
    <p class="muted" style="font-size:12px;margin-top:8px">${cards.length} lignes · cliquez un titre pour ouvrir la carte · durée/charge/% éditables inline</p>`;

  host.querySelectorAll('th[data-sort]').forEach((th) => {
    th.onclick = () => {
      const k = th.dataset.sort;
      if (sortKey === k) sortDir *= -1;
      else { sortKey = k; sortDir = 1; }
      renderTable();
    };
  });
  host.querySelectorAll('tr[data-card] td:nth-child(2)').forEach((td) => {
    td.style.cursor = 'pointer';
    td.onclick = () => openCardModal(td.closest('tr').dataset.card, { onChange: rerender });
  });
  host.querySelectorAll('.cell-edit').forEach((input) => {
    input.addEventListener('change', async () => {
      const cardId = input.closest('tr').dataset.card;
      const field = input.dataset.num;
      const v = input.value.trim();
      const patch = { [field]: v === '' ? null : Number(v) };
      try {
        await api.updateCard(cardId, patch);
        await hydrateLabelIds();
      } catch (e) {
        toast(e.message, 'err');
        rerender();
      }
    });
  });
}

async function rerender() {
  await hydrateLabelIds();
  renderTable();
}
