// Modale de carte : détail complet + actions IA.

import { api } from '../api.js';
import { escapeHtml, mdToHtml, fmtDateTime, fmtDate, parseDate, toISODate, PRIORITY_META, LABEL_COLORS } from '../format.js';
import { openModal, toast, confirmDialog, promptDialog } from '../ui/modal.js';
import { state } from '../state.js';
import { on } from '../ws.js';

export async function openCardModal(cardId, { onChange } = {}) {
  let detail;
  try {
    detail = await api.card(cardId);
  } catch (e) {
    return toast(e.message, 'err');
  }
  let checklists = detail.checklists || [];
  let comments = detail.comments || [];
  let attachments = detail.attachments || [];
  let fieldValues = detail.custom_fields || {};
  const schedule = detail.schedule;

  const notify = () => onChange?.();

  function render() {
    const card = detail.card;
    const labelIds = (detail.labels || []).map((l) => l.id);
    const pct = progressOf(checklists);
    const { el, close } = openModal(`
      <div class="modal-head">
        <h2 data-edit="title">${escapeHtml(card.title)}</h2>
        <button class="iconbtn" id="cm-ai" title="Assistant IA">✨</button>
        <button class="iconbtn" data-close title="Fermer">✕</button>
      </div>
      <div class="modal-body">
        <div class="grid-2">
          <div>
            <div class="modal-section">
              <div class="label">Description</div>
              <div id="cm-desc-view" class="md">${mdToHtml(card.description) || '<span class="muted">Aucune description</span>'}</div>
              <textarea class="input" id="cm-desc" style="display:none;min-height:120px"></textarea>
              <div class="row" id="cm-desc-actions" style="display:none;margin-top:8px;justify-content:flex-end">
                <button class="btn sm" id="cm-desc-cancel">Annuler</button>
                <button class="btn sm primary" id="cm-desc-save">Enregistrer</button>
              </div>
            </div>

            <div class="modal-section">
              <div class="label">Checklists ${pct !== null ? `<span class="muted">· ${pct}%</span>` : ''}</div>
              ${pct !== null ? `<div class="progress-track"><div class="progress-fill" style="width:${pct}%"></div></div>` : ''}
              <div id="cm-checklists">
                ${checklists.map((cl) => `
                  <div class="muted" style="margin:8px 0 2px;font-weight:600">${escapeHtml(cl.title)}
                    <button class="iconbtn" data-del-cl="${cl.id}" title="Supprimer la checklist" style="width:20px;height:20px;font-size:12px">🗑</button>
                  </div>
                  ${cl.items.map((it) => `
                    <div class="checkline">
                      <input type="checkbox" data-item="${it.id}" ${it.checked ? 'checked' : ''} />
                      <span class="t ${it.checked ? 'done' : ''}">${escapeHtml(it.title)}</span>
                      <button class="iconbtn" data-del-item="${it.id}" style="width:20px;height:20px;font-size:11px">✕</button>
                    </div>`).join('')}
                  <button class="btn ghost sm" data-add-item="${cl.id}">＋ élément</button>
                `).join('')}
              </div>
              <button class="btn ghost sm" id="cm-add-checklist">＋ checklist</button>
            </div>

            <div class="modal-section">
              <div class="label">Commentaires</div>
              <div class="row">
                <input class="input" id="cm-comment" placeholder="Ajouter un commentaire…" />
                <button class="btn primary sm" id="cm-comment-add" style="flex:none">Envoyer</button>
              </div>
              <div id="cm-comments" style="margin-top:10px">
                ${comments.map((c) => `
                  <div class="comment">
                    <div class="who">${fmtDateTime(c.created_at)} <button class="iconbtn" data-del-com="${c.id}" style="width:18px;height:18px;font-size:10px">🗑</button></div>
                    <div>${escapeHtml(c.body)}</div>
                  </div>`).join('') || '<span class="muted">Aucun commentaire</span>'}
              </div>
            </div>
          </div>

          <div class="side">
            <div class="modal-section">
              <div class="label">Attributs</div>
              <div class="field"><label>Échéance</label><input type="date" class="input" id="cm-due" value="${dateOnly(card.due)}" /></div>
              <div class="row">
                <div class="field"><label>Heure</label><input type="time" class="input" id="cm-due-time" value="${timeOnly(card.due)}" /></div>
                <div class="field"><label>Terminée</label>
                  <input type="checkbox" id="cm-due-complete" ${card.due_complete ? 'checked' : ''} style="width:16px;height:16px;margin-top:8px" />
                </div>
              </div>
              <div class="field"><label>Début</label><input type="date" class="input" id="cm-start" value="${dateOnly(card.start)}" /></div>
              <div class="field"><label>Priorité</label>
                <select class="input" id="cm-priority">
                  ${Object.entries(PRIORITY_META).map(([k, v]) => `<option value="${k}" ${card.priority === k ? 'selected' : ''}>${v.label}</option>`).join('')}
                </select>
              </div>
              <div class="row">
                <div class="field"><label>Durée (j ouvrés)</label><input type="number" step="0.5" min="0" class="input" id="cm-duration" value="${card.duration_days ?? ''}" /></div>
                <div class="field"><label>Charge (h)</label><input type="number" step="0.5" min="0" class="input" id="cm-work" value="${card.work_hours ?? ''}" /></div>
              </div>
              <div class="row">
                <div class="field"><label>% accompli</label><input type="number" min="0" max="100" class="input" id="cm-pct" value="${card.pct_complete}" /></div>
                <div class="field"><label>Jalon</label>
                  <input type="checkbox" id="cm-milestone" ${card.is_milestone ? 'checked' : ''} style="width:16px;height:16px;margin-top:8px" />
                </div>
              </div>
            </div>

            <div class="modal-section">
              <div class="label">Labels</div>
              <div id="cm-labels">
                ${state.labels.map((l) => `
                  <label class="checkline"><input type="checkbox" data-label="${l.id}" ${labelIds.includes(l.id) ? 'checked' : ''} />
                    <span class="chip-label" style="background:${l.color}">${escapeHtml(l.name || '•')}</span></label>`).join('') || '<span class="muted">Aucun label sur ce board</span>'}
              </div>
            </div>

            ${state.fields.length ? `
            <div class="modal-section">
              <div class="label">Champs personnalisés</div>
              ${state.fields.map((f) => `
                <div class="field"><label>${escapeHtml(f.name)}</label>${fieldInput(f, fieldValues[f.id])}</div>`).join('')}
            </div>` : ''}

            <div class="modal-section">
              <div class="label">Couverture</div>
              <div class="swatches" id="cm-covers">
                <button data-cover="" title="Aucune" style="background:var(--surface-2)">✕</button>
                ${['#6366f1', '#22d3ee', '#34d399', '#f59e0b', '#ef4444', '#f472b6'].map((c) =>
                  `<button data-cover="${c}" style="background:${c}"></button>`).join('')}
              </div>
            </div>

            <div class="modal-section">
              <div class="label">Pièces jointes</div>
              <input type="file" id="cm-file" class="input" style="padding:5px" />
              <div id="cm-attachments" style="margin-top:8px">
                ${attachments.map((a) => `
                  <div class="row" style="margin-bottom:4px">
                    <span style="flex:1;overflow:hidden;text-overflow:ellipsis">📎 <a href="/api/v1/attachments/${a.id}/download" download>${escapeHtml(a.filename)}</a></span>
                    <button class="btn ghost sm" data-cover-att="${a.id}">cover</button>
                    <button class="iconbtn" data-del-att="${a.id}" style="width:20px;height:20px;font-size:11px">✕</button>
                  </div>`).join('') || ''}
              </div>
            </div>

            <div class="modal-section">
              <div class="label">Ordonnancement</div>
              ${schedule ? `
                <div style="font-size:12.5px">
                  <div>Début au plus tôt : <b>${fmtDate(schedule.es) || '—'}</b></div>
                  <div>Fin au plus tard : <b>${fmtDate(schedule.ef) || '—'}</b></div>
                  <div>Marge totale : <b>${schedule.total_float_days ?? '—'} j</b> ${schedule.is_critical ? '<span class="badge overdue">chemin critique</span>' : ''}</div>
                </div>` : '<span class="muted">Aucune donnée CPM (définissez des durées/dépendances)</span>'}
              <button class="btn ghost sm" id="cm-add-dep" style="margin-top:8px">＋ dépendance</button>
              <div id="cm-deps" style="margin-top:6px;font-size:12.5px">
                ${renderDeps(detail)}
              </div>
            </div>

            <div class="row" style="justify-content:flex-end">
              <button class="btn ghost sm" id="cm-duplicate">Dupliquer</button>
              ${card.archived ? '<button class="btn sm" id="cm-unarchive">Désarchiver</button>' : '<button class="btn sm" id="cm-archive">Archiver</button>'}
              <button class="btn danger sm" id="cm-delete">Supprimer</button>
            </div>
          </div>
        </div>
      </div>`);

    bind(el, close);
    return { el, close };
  }

  function renderDeps(detail) {
    const rows = [];
    for (const d of detail.dependencies_in || []) {
      const p = state.cards.find((c) => c.id === d.predecessor_id);
      rows.push(depRow(d, p?.title || d.predecessor_id.slice(0, 10), 'prédécesseur'));
    }
    for (const d of detail.dependencies_out || []) {
      const s = state.cards.find((c) => c.id === d.successor_id);
      rows.push(depRow(d, s?.title || d.successor_id.slice(0, 10), 'successeur'));
    }
    return rows.join('') || '<span class="muted">Aucune dépendance</span>';
  }

  function depRow(d, otherTitle, role) {
    return `<div class="row" style="margin-bottom:3px">
      <span style="flex:1;overflow:hidden;text-overflow:ellipsis">${d.type} · ${escapeHtml(otherTitle)} <span class="muted">(${role}${d.lag_days ? `, lag ${d.lag_days}j` : ''})</span></span>
      <button class="iconbtn" data-del-dep="${d.id}" style="width:20px;height:20px;font-size:11px">✕</button>
    </div>`;
  }

  async function refreshData() {
    const d = await api.card(detail.card.id);
    detail = d;
    checklists = d.checklists || [];
    comments = d.comments || [];
    attachments = d.attachments || [];
    fieldValues = d.custom_fields || {};
    const scroll = current?.el.scrollTop;
    current = render();
    if (scroll) current.el.scrollTop = scroll;
    notify();
  }

  let current = render();

  function progressOf(cls) {
    let total = 0, done = 0;
    for (const cl of cls) for (const it of cl.items) { total += 1; if (it.checked) done += 1; }
    return total ? Math.round((done / total) * 100) : null;
  }

  function dateOnly(s) {
    const d = parseDate(s);
    return d ? toISODate(d) : '';
  }
  function timeOnly(s) {
    if (!s || /^\d{4}-\d{2}-\d{2}$/.test(s)) return '';
    const d = parseDate(s);
    if (!d || (d.getHours() === 0 && d.getMinutes() === 0)) return '';
    return `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`;
  }

  function fieldInput(f, value) {
    const v = value ?? '';
    if (f.kind === 'select') {
      let opts = [];
      try { opts = JSON.parse(f.options); } catch { /* ignore */ }
      return `<select class="input" data-field="${f.id}"><option value=""></option>${opts.map((o) => `<option ${v === o ? 'selected' : ''}>${escapeHtml(o)}</option>`).join('')}</select>`;
    }
    if (f.kind === 'checkbox') {
      return `<input type="checkbox" data-field="${f.id}" ${v === 'true' ? 'checked' : ''} style="width:16px;height:16px" />`;
    }
    if (f.kind === 'number') return `<input type="number" class="input" data-field="${f.id}" value="${escapeHtml(v)}" />`;
    if (f.kind === 'date') return `<input type="date" class="input" data-field="${f.id}" value="${escapeHtml(v)}" />`;
    return `<input class="input" data-field="${f.id}" value="${escapeHtml(v)}" />`;
  }

  function bind(el, close) {
    const card = detail.card;
    // --- titre inline
    const title = el.querySelector('[data-edit=title]');
    title.addEventListener('click', async () => {
      const nv = await promptDialog('Modifier le titre', card.title);
      if (nv && nv !== card.title) {
        detail.card = await api.updateCard(card.id, { title: nv });
        refreshData();
      }
    });
    // --- description
    const descView = el.querySelector('#cm-desc-view');
    const descEdit = el.querySelector('#cm-desc');
    const descActions = el.querySelector('#cm-desc-actions');
    descView.addEventListener('click', () => {
      descView.style.display = 'none';
      descEdit.style.display = '';
      descActions.style.display = '';
      descEdit.value = card.description;
      descEdit.focus();
    });
    el.querySelector('#cm-desc-cancel').onclick = () => { render(); };
    el.querySelector('#cm-desc-save').onclick = async () => {
      detail.card = await api.updateCard(card.id, { description: descEdit.value });
      refreshData();
    };
    // --- attributs
    const upd = (patch) => api.updateCard(card.id, patch).then((c) => { detail.card = c; notify(); }).catch((e) => toast(e.message, 'err'));
    el.querySelector('#cm-due').onchange = (e) => {
      const time = el.querySelector('#cm-due-time').value;
      upd({ due: e.target.value ? (e.target.value + (time ? `T${time}:00` : '')) : null });
    };
    el.querySelector('#cm-due-time').onchange = (e) => {
      const d = el.querySelector('#cm-due').value;
      if (d) upd({ due: d + (e.target.value ? `T${e.target.value}:00` : '') });
    };
    el.querySelector('#cm-due-complete').onchange = (e) => upd({ due_complete: e.target.checked });
    el.querySelector('#cm-start').onchange = (e) => upd({ start: e.target.value || null });
    el.querySelector('#cm-priority').onchange = (e) => upd({ priority: e.target.value });
    el.querySelector('#cm-duration').onchange = (e) => upd({ duration_days: e.target.value === '' ? null : Number(e.target.value) });
    el.querySelector('#cm-work').onchange = (e) => upd({ work_hours: e.target.value === '' ? null : Number(e.target.value) });
    el.querySelector('#cm-pct').onchange = (e) => upd({ pct_complete: Number(e.target.value) });
    el.querySelector('#cm-milestone').onchange = (e) => upd({ is_milestone: e.target.checked });
    // --- labels
    el.querySelector('#cm-labels').addEventListener('change', async () => {
      const ids = [...el.querySelectorAll('[data-label]:checked')].map((c) => c.dataset.label);
      try {
        await api.setCardLabels(card.id, ids);
        detail.labels = ids.map((id) => state.labels.find((l) => l.id === id)).filter(Boolean);
        notify();
      } catch (e) { toast(e.message, 'err'); }
    });
    // --- champs personnalisés
    el.querySelectorAll('[data-field]').forEach((input) => {
      input.addEventListener('change', async () => {
        const fid = input.dataset.field;
        const value = input.type === 'checkbox' ? String(input.checked) : input.value;
        try {
          await api.setFieldValue(card.id, fid, value);
          fieldValues[fid] = value;
          notify();
        } catch (e) { toast(e.message, 'err'); }
      });
    });
    // --- couverture
    el.querySelector('#cm-covers').addEventListener('click', async (e) => {
      const c = e.target.closest('[data-cover]')?.dataset.cover;
      if (c === undefined) return;
      await api.setCover(card.id, c || null);
      refreshData();
    });
    // --- checklists
    el.querySelector('#cm-add-checklist').onclick = async () => {
      const title = await promptDialog('Titre de la checklist', 'Étapes');
      if (!title) return;
      await api.createChecklist(card.id, { title });
      refreshData();
    };
    el.querySelector('#cm-checklists').addEventListener('click', async (e) => {
      const addBtn = e.target.closest('[data-add-item]');
      if (addBtn) {
        const t = await promptDialog('Nouvel élément');
        if (t) { await api.createChecklistItem(addBtn.dataset.addItem, { title: t }); refreshData(); }
        return;
      }
      const delItem = e.target.closest('[data-del-item]');
      if (delItem) { await api.deleteChecklistItem(delItem.dataset.delItem); refreshData(); return; }
      const delCl = e.target.closest('[data-del-cl]');
      if (delCl) {
        if (await confirmDialog('Supprimer cette checklist ?')) { await api.deleteChecklist(delCl.dataset.delCl); refreshData(); }
      }
    });
    el.querySelector('#cm-checklists').addEventListener('change', async (e) => {
      const cb = e.target.closest('[data-item]');
      if (cb) { await api.updateChecklistItem(cb.dataset.item, { checked: cb.checked }); refreshData(); }
    });
    // --- commentaires
    const addComment = async () => {
      const input = el.querySelector('#cm-comment');
      if (!input.value.trim()) return;
      await api.addComment(card.id, input.value);
      input.value = '';
      refreshData();
    };
    el.querySelector('#cm-comment-add').onclick = addComment;
    el.querySelector('#cm-comment').addEventListener('keydown', (e) => { if (e.key === 'Enter') addComment(); });
    el.querySelector('#cm-comments').addEventListener('click', async (e) => {
      const del = e.target.closest('[data-del-com]');
      if (del) { await api.deleteComment(del.dataset.delCom); refreshData(); }
    });
    // --- pièces jointes
    el.querySelector('#cm-file').addEventListener('change', async (e) => {
      const file = e.target.files[0];
      if (!file) return;
      try {
        await api.uploadAttachment(card.id, file);
        refreshData();
      } catch (err) { toast(err.message, 'err'); }
    });
    el.querySelector('#cm-attachments').addEventListener('click', async (e) => {
      const del = e.target.closest('[data-del-att]');
      if (del) { await api.deleteAttachment(del.dataset.delAtt); refreshData(); return; }
      const cov = e.target.closest('[data-cover-att]');
      if (cov) { await api.setAttachmentCover(cov.dataset.coverAtt); refreshData(); }
    });
    // --- dépendances
    el.querySelector('#cm-add-dep').onclick = async () => {
      if (!state.board) return;
      const predId = await promptDialog('ID de la carte prédécesseur (ou tapez pour chercher)');
      if (!predId) return;
      const hit = state.cards.find((c) => c.title.toLowerCase() === predId.toLowerCase()) || state.cards.find((c) => c.id === predId);
      if (!hit) return toast('Carte introuvable', 'err');
      try {
        await api.addDep(state.board.id, { predecessor_id: hit.id, successor_id: card.id, type: 'FS' });
        refreshData();
      } catch (e) { toast(e.message, 'err'); }
    };
    el.querySelector('#cm-deps').addEventListener('click', async (e) => {
      const del = e.target.closest('[data-del-dep]');
      if (del) { await api.deleteDep(del.dataset.delDep); refreshData(); }
    });
    // --- bas de modale
    el.querySelector('#cm-duplicate').onclick = async () => {
      await api.duplicateCard(card.id);
      close();
      notify();
    };
    el.querySelector('#cm-archive')?.addEventListener('click', async () => {
      await api.updateCard(card.id, { archived: true });
      close(); notify();
    });
    el.querySelector('#cm-unarchive')?.addEventListener('click', async () => {
      await api.updateCard(card.id, { archived: false });
      close(); notify();
    });
    el.querySelector('#cm-delete').onclick = async () => {
      if (await confirmDialog('Supprimer définitivement cette carte ?')) {
        await api.deleteCard(card.id);
        close(); notify();
      }
    };
    // --- IA
    el.querySelector('#cm-ai').onclick = async () => {
      const choice = await Promise.race([]);
      const menu = openModal(`
        <div class="modal-head"><h2>✨ Assistant IA</h2></div>
        <div class="modal-body">
          <div class="row" style="flex-direction:column;gap:8px">
            <button class="btn" data-ai="checklist">Générer une checklist</button>
            <button class="btn" data-ai="rewrite">Reformuler la description</button>
            <button class="btn" data-ai="due">Suggérer une échéance</button>
          </div>
        </div>`);
      menu.el.querySelector('.modal-body').addEventListener('click', async (e) => {
        const what = e.target.closest('[data-ai]')?.dataset.ai;
        if (!what) return;
        menu.close();
        try {
          if (what === 'checklist') {
            await api.aiChecklist(card.id, true);
            toast('Checklist générée', 'ok');
          } else if (what === 'rewrite') {
            const r = await api.aiRewrite(card.description || card.title);
            detail.card = await api.updateCard(card.id, { description: r.text });
          } else if (what === 'due') {
            const r = await api.aiParseTask(`Planifier : ${card.title}. Propose une échéance réaliste dans les 2 semaines.`, detail.card.list_id, false);
            const due = r.parsed?.due;
            if (due) { detail.card = await api.updateCard(card.id, { due }); toast(`Échéance : ${fmtDate(due)}`, 'ok'); }
          }
          refreshData();
        } catch (err) { toast(err.message, 'err'); }
      });
    };
  }
}
