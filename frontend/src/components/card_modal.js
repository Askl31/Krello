// Modale de carte : détail complet + actions IA.

import { api } from '../api.js';
import { escapeHtml, mdToHtml, fmtDateTime, fmtDate, parseDate, toISODate, PRIORITY_META, LABEL_COLORS } from '../format.js';
import { openModal, toast, confirmDialog, promptDialog } from '../ui/modal.js';
import { state } from '../state.js';
import { on } from '../ws.js';
import { icon } from '../icons.js';

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
    const currentCover = getCoverValue(card.cover);

    const { el, close } = openModal(`
      <div class="modal-head">
        <h2 data-edit="title" title="Cliquez pour modifier le titre" style="cursor:pointer">${escapeHtml(card.title)}</h2>
        ${state.aiStatus?.available ? `
          <button class="iconbtn" id="cm-ai" title="Assistant IA Apple Intelligence">
            ${icon('ai', { size: 16 })}
          </button>` : ''}
        <button class="iconbtn" data-close title="Fermer">${icon('close', { size: 16 })}</button>
      </div>
      <div class="modal-body">
        <div class="grid-2">
          <div>
            <div class="modal-section">
              <div class="label" style="display:flex;justify-content:space-between">
                <span>Description</span>
                <span class="muted" style="text-transform:none;font-weight:normal">Markdown supporté</span>
              </div>
              <div id="cm-desc-view" class="md" style="cursor:pointer;padding:8px 10px;border-radius:var(--radius-sm);border:1px dashed var(--border);min-height:50px;background:var(--surface-2)">
                ${mdToHtml(card.description) || '<span class="muted">Cliquez pour ajouter une description…</span>'}
              </div>
              <textarea class="input" id="cm-desc" style="display:none;min-height:120px"></textarea>
              <div class="row" id="cm-desc-actions" style="display:none;margin-top:8px;justify-content:flex-end;gap:8px">
                <button class="btn sm" id="cm-desc-cancel">Annuler</button>
                <button class="btn sm primary" id="cm-desc-save">Enregistrer</button>
              </div>
            </div>

            <div class="modal-section">
              <div class="label" style="display:flex;justify-content:space-between;align-items:center">
                <span>Checklists</span>
                ${pct !== null ? `<span class="badge" style="color:var(--success)">${pct}%</span>` : ''}
              </div>
              ${pct !== null ? `<div class="progress-track"><div class="progress-fill" style="width:${pct}%"></div></div>` : ''}
              <div id="cm-checklists">
                ${checklists.map((cl) => `
                  <div style="display:flex;align-items:center;justify-content:space-between;margin:10px 0 4px">
                    <b style="font-size:13px">${escapeHtml(cl.title)}</b>
                    <button class="iconbtn" data-del-cl="${cl.id}" title="Supprimer la checklist" style="width:22px;height:22px">
                      ${icon('trash', { size: 13 })}
                    </button>
                  </div>
                  ${cl.items.map((it) => `
                    <div class="checkline">
                      <input type="checkbox" data-item="${it.id}" ${it.checked ? 'checked' : ''} />
                      <span class="t ${it.checked ? 'done' : ''}">${escapeHtml(it.title)}</span>
                      <button class="iconbtn" data-del-item="${it.id}" title="Supprimer" style="width:20px;height:20px">
                        ${icon('close', { size: 12 })}
                      </button>
                    </div>`).join('')}
                  <button class="btn ghost sm" data-add-item="${cl.id}" style="margin-top:4px">
                    ${icon('plus', { size: 12 })} <span>Ajouter un élément</span>
                  </button>
                `).join('')}
              </div>
              <button class="btn ghost sm" id="cm-add-checklist" style="margin-top:8px">
                ${icon('plus', { size: 13 })} <span>Nouvelle checklist</span>
              </button>
            </div>

            <div class="modal-section">
              <div class="label">Commentaires</div>
              <div class="row" style="gap:8px">
                <input class="input" id="cm-comment" placeholder="Écrire un commentaire…" />
                <button class="btn primary sm" id="cm-comment-add" style="flex:none">Envoyer</button>
              </div>
              <div id="cm-comments" style="margin-top:12px">
                ${comments.map((c) => `
                  <div class="comment">
                    <div class="who">
                      <span>${fmtDateTime(c.created_at)}</span>
                      <button class="iconbtn" data-del-com="${c.id}" title="Supprimer" style="width:18px;height:18px">
                        ${icon('trash', { size: 11 })}
                      </button>
                    </div>
                    <div>${escapeHtml(c.body)}</div>
                  </div>`).join('') || '<div class="muted" style="font-size:12.5px;padding:6px 0">Aucun commentaire</div>'}
              </div>
            </div>
          </div>

          <div class="side">
            <div class="modal-section">
              <div class="label">Planification & Délais</div>
              <div class="field">
                <label>Échéance</label>
                <input type="date" class="input" id="cm-due" value="${dateOnly(card.due)}" />
              </div>
              <div class="row">
                <div class="field">
                  <label>Heure</label>
                  <input type="time" class="input" id="cm-due-time" value="${timeOnly(card.due)}" />
                </div>
                <div class="field" style="flex:none">
                  <label>Terminée</label>
                  <input type="checkbox" id="cm-due-complete" ${card.due_complete ? 'checked' : ''} style="width:17px;height:17px;margin-top:6px" />
                </div>
              </div>
              <div class="field">
                <label>Date de début</label>
                <input type="date" class="input" id="cm-start" value="${dateOnly(card.start)}" />
              </div>
              <div class="field">
                <label>Priorité</label>
                <select class="input" id="cm-priority">
                  ${Object.entries(PRIORITY_META).map(([k, v]) => `<option value="${k}" ${card.priority === k ? 'selected' : ''}>${v.label}</option>`).join('')}
                </select>
              </div>
              <div class="row">
                <div class="field">
                  <label>Durée (j ouvrés)</label>
                  <input type="number" step="0.5" min="0" class="input" id="cm-duration" value="${card.duration_days ?? ''}" />
                </div>
                <div class="field">
                  <label>Charge (h)</label>
                  <input type="number" step="0.5" min="0" class="input" id="cm-work" value="${card.work_hours ?? ''}" />
                </div>
              </div>
              <div class="row">
                <div class="field">
                  <label>% accompli</label>
                  <input type="number" min="0" max="100" class="input" id="cm-pct" value="${card.pct_complete}" />
                </div>
                <div class="field" style="flex:none">
                  <label>Jalon</label>
                  <input type="checkbox" id="cm-milestone" ${card.is_milestone ? 'checked' : ''} style="width:17px;height:17px;margin-top:6px" />
                </div>
              </div>
            </div>

            <div class="modal-section">
              <div class="label">Labels</div>
              <div id="cm-labels">
                ${state.labels.map((l) => `
                  <label class="checkline" style="cursor:pointer">
                    <input type="checkbox" data-label="${l.id}" ${labelIds.includes(l.id) ? 'checked' : ''} />
                    <span class="chip-label" style="background:${l.color}">${escapeHtml(l.name || '•')}</span>
                  </label>`).join('') || '<span class="muted">Aucun label sur ce board</span>'}
              </div>
            </div>

            ${state.fields.length ? `
            <div class="modal-section">
              <div class="label">Champs personnalisés</div>
              ${state.fields.map((f) => `
                <div class="field"><label>${escapeHtml(f.name)}</label>${fieldInput(f, fieldValues[f.id])}</div>`).join('')}
            </div>` : ''}

            <div class="modal-section">
              <div class="label">Couleur de couverture</div>
              <div class="swatches" id="cm-covers">
                <button data-cover="" class="${!currentCover ? 'sel' : ''}" title="Aucune" style="background:var(--surface-2);color:var(--muted)">✕</button>
                ${['#6366f1', '#06b6d4', '#10b981', '#f59e0b', '#ef4444', '#ec4899', '#8b5cf6'].map((c) =>
                  `<button data-cover="${c}" class="${currentCover === c ? 'sel' : ''}" style="background:${c}"></button>`).join('')}
              </div>
            </div>

            <div class="modal-section">
              <div class="label">Pièces jointes</div>
              <input type="file" id="cm-file" class="input" style="padding:6px;font-size:12px" />
              <div id="cm-attachments" style="margin-top:8px">
                ${attachments.map((a) => `
                  <div class="row" style="margin-bottom:6px;padding:4px 6px;border-radius:var(--radius-xs);background:var(--surface-2)">
                    <span style="flex:1;overflow:hidden;text-overflow:ellipsis;font-size:12px">
                      📎 <a href="/api/v1/attachments/${a.id}/download" download>${escapeHtml(a.filename)}</a>
                    </span>
                    <button class="btn ghost sm" data-cover-att="${a.id}">cover</button>
                    <button class="iconbtn" data-del-att="${a.id}" title="Supprimer" style="width:20px;height:20px">
                      ${icon('close', { size: 12 })}
                    </button>
                  </div>`).join('') || ''}
              </div>
            </div>

            <div class="modal-section">
              <div class="label">Ordonnancement CPM</div>
              ${schedule ? `
                <div style="font-size:12px;background:var(--surface-2);padding:8px 10px;border-radius:var(--radius-sm)">
                  <div>Début au plus tôt : <b>${fmtDate(schedule.es) || '—'}</b></div>
                  <div style="margin-top:3px">Fin au plus tard : <b>${fmtDate(schedule.ef) || '—'}</b></div>
                  <div style="margin-top:3px">Marge totale : <b>${schedule.total_float_days ?? '—'} j</b> ${schedule.is_critical ? '<span class="badge overdue" style="margin-left:6px">chemin critique</span>' : ''}</div>
                </div>` : '<span class="muted" style="font-size:12px">Définissez des durées ou dépendances pour le calcul CPM</span>'}
              <button class="btn ghost sm" id="cm-add-dep" style="margin-top:8px">
                ${icon('plus', { size: 12 })} <span>Ajouter une dépendance</span>
              </button>
              <div id="cm-deps" style="margin-top:8px;font-size:12px">
                ${renderDeps(detail)}
              </div>
            </div>

            <div class="row" style="justify-content:flex-end;margin-top:20px;padding-top:12px;border-top:1px solid var(--border)">
              <button class="btn ghost sm" id="cm-duplicate" title="Créer une copie">${icon('copy', { size: 13 })} <span>Dupliquer</span></button>
              ${card.archived ? '<button class="btn sm" id="cm-unarchive">Désarchiver</button>' : '<button class="btn sm" id="cm-archive">Archiver</button>'}
              <button class="btn danger sm" id="cm-delete">${icon('trash', { size: 13 })} <span>Supprimer</span></button>
            </div>
          </div>
        </div>
      </div>`, { id: 'card-modal', size: 'lg' });

    bind(el, close);
    return { el, close };
  }

  function getCoverValue(coverJson) {
    try {
      const c = JSON.parse(coverJson);
      if (c.type === 'color') return c.value;
    } catch { /* ignore */ }
    return null;
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
    return rows.join('') || '<span class="muted">Aucune dépendance active</span>';
  }

  function depRow(d, otherTitle, role) {
    return `<div class="row" style="margin-bottom:4px;padding:3px 6px;border-radius:var(--radius-xs);background:var(--surface-2)">
      <span style="flex:1;overflow:hidden;text-overflow:ellipsis">${d.type} · ${escapeHtml(otherTitle)} <span class="muted">(${role}${d.lag_days ? `, lag ${d.lag_days}j` : ''})</span></span>
      <button class="iconbtn" data-del-dep="${d.id}" title="Supprimer" style="width:20px;height:20px">
        ${icon('close', { size: 12 })}
      </button>
    </div>`;
  }

  async function refreshData() {
    const d = await api.card(detail.card.id);
    detail = d;
    checklists = d.checklists || [];
    comments = d.comments || [];
    attachments = d.attachments || [];
    fieldValues = d.custom_fields || {};

    const descEl = current?.el.querySelector('#cm-desc');
    const descEditing = descEl && descEl.style.display !== 'none' ? descEl.value : null;
    const scroll = current?.el.scrollTop;
    current = render();
    if (descEditing !== null) {
      const nv = current.el.querySelector('#cm-desc');
      const view = current.el.querySelector('#cm-desc-view');
      const actions = current.el.querySelector('#cm-desc-actions');
      if (nv && view && actions) {
        view.style.display = 'none';
        nv.style.display = '';
        actions.style.display = '';
        nv.value = descEditing;
        nv.focus();
      }
    }
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
      const predId = await promptDialog('ID de la carte prédécesseur (ou tapez le nom)');
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
    el.querySelector('#cm-ai')?.addEventListener('click', () => {
      const menu = openModal(`
        <div class="modal-head">
          <h2>✨ Assistant IA Apple Intelligence</h2>
          <button class="iconbtn" data-close title="Fermer">${icon('close', { size: 16 })}</button>
        </div>
        <div class="modal-body">
          <div class="row" style="flex-direction:column;gap:10px">
            <button class="btn" data-ai="checklist" style="justify-content:flex-start">
              ${icon('check', { size: 15 })} <span>Générer une checklist intelligente</span>
            </button>
            <button class="btn" data-ai="rewrite" style="justify-content:flex-start">
              ${icon('ai', { size: 15 })} <span>Reformuler et enrichir la description</span>
            </button>
            <button class="btn" data-ai="due" style="justify-content:flex-start">
              ${icon('clock', { size: 15 })} <span>Suggérer une échéance réaliste</span>
            </button>
          </div>
        </div>`);
      menu.el.querySelector('.modal-body').addEventListener('click', async (e) => {
        const what = e.target.closest('[data-ai]')?.dataset.ai;
        if (!what) return;
        menu.close();
        try {
          if (what === 'checklist') {
            await api.aiChecklist(card.id, true);
            toast('Checklist générée par Apple Intelligence', 'ok');
          } else if (what === 'rewrite') {
            const r = await api.aiRewrite(card.description || card.title);
            detail.card = await api.updateCard(card.id, { description: r.text });
            toast('Description reformulée', 'ok');
          } else if (what === 'due') {
            const r = await api.aiParseTask(`Planifier : ${card.title}. Propose une échéance réaliste dans les 2 semaines.`, detail.card.list_id, false);
            const due = r.parsed?.due;
            if (due) {
              detail.card = await api.updateCard(card.id, { due });
              toast(`Échéance suggérée : ${fmtDate(due)}`, 'ok');
            }
          }
          refreshData();
        } catch (err) { toast(err.message, 'err'); }
      });
    });
  }
}
