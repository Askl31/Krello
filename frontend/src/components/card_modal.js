// Modale de carte : détail complet + actions IA.

import { api } from '../api.js';
import { escapeHtml, mdToHtml, fmtDateTime, fmtDate, parseDate, toISODate, PRIORITY_META, LABEL_COLORS } from '../format.js';
import { openModal, toast, confirmDialog, promptDialog } from '../ui/modal.js';
import { state, visibleLists, labelById } from '../state.js';
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
  let schedule = detail.schedule;

  const notify = () => onChange?.();

  function progressOf(cls) {
    let total = 0, done = 0;
    for (const cl of cls) {
      for (const it of cl.items || []) {
        total += 1;
        if (it.checked) done += 1;
      }
    }
    return total ? Math.round((done / total) * 100) : null;
  }

  function renderChecklistItems(cls) {
    return cls.map((cl) => `
      <div style="display:flex;align-items:center;justify-content:space-between;margin:10px 0 4px">
        <b style="font-size:13px">${escapeHtml(cl.title)}</b>
        <button class="iconbtn" data-del-cl="${cl.id}" title="Supprimer la checklist" style="width:22px;height:22px">
          ${icon('trash', { size: 13 })}
        </button>
      </div>
      ${(cl.items || []).map((it) => `
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
    `).join('');
  }

  function updateChecklistProgress(container, cls) {
    const pct = progressOf(cls);
    const badge = container.querySelector('#cm-checklists-badge');
    const track = container.querySelector('#cm-checklists-track');
    const fill = container.querySelector('#cm-checklists-fill');
    if (pct !== null) {
      if (badge) {
        badge.textContent = `${pct}%`;
        badge.style.display = '';
      }
      if (track) track.style.display = '';
      if (fill) {
        fill.style.width = `${pct}%`;
        fill.classList.toggle('done', pct === 100);
      }
    } else {
      if (badge) badge.style.display = 'none';
      if (track) track.style.display = 'none';
      if (fill) {
        fill.style.width = '0%';
        fill.classList.remove('done');
      }
    }
  }

  function renderComments(coms) {
    return coms.map((c) => `
      <div class="comment">
        <div class="who">
          <span>${fmtDateTime(c.created_at)}</span>
          <button class="iconbtn" data-del-com="${c.id}" title="Supprimer" style="width:18px;height:18px">
            ${icon('trash', { size: 11 })}
          </button>
        </div>
        <div>${escapeHtml(c.body)}</div>
      </div>`).join('') || '<div class="muted" style="font-size:12.5px;padding:6px 0">Aucun commentaire</div>';
  }

  function renderAttachments(atts) {
    return atts.map((a) => `
      <div class="row" style="margin-bottom:6px;padding:4px 6px;border-radius:var(--radius-xs);background:var(--surface-2)">
        <span style="flex:1;overflow:hidden;text-overflow:ellipsis;font-size:12px">
          📎 <a href="/api/v1/attachments/${a.id}/download" download>${escapeHtml(a.filename)}</a>
        </span>
        <button class="btn ghost sm" data-cover-att="${a.id}">cover</button>
        <button class="iconbtn" data-del-att="${a.id}" title="Supprimer" style="width:20px;height:20px">
          ${icon('close', { size: 12 })}
        </button>
      </div>`).join('');
  }

  function renderCpmSchedule(sched) {
    if (!sched) return '<span class="muted" style="font-size:12px">Définissez des durées ou dépendances pour le calcul CPM</span>';
    return `<div style="font-size:12px;background:var(--surface-2);padding:8px 10px;border-radius:var(--radius-sm)">
      <div>Début au plus tôt : <b>${fmtDate(sched.es) || '—'}</b></div>
      <div style="margin-top:3px">Fin au plus tard : <b>${fmtDate(sched.ef) || '—'}</b></div>
      <div style="margin-top:3px">Marge totale : <b>${sched.total_float_days ?? '—'} j</b> ${sched.is_critical ? '<span class="badge overdue" style="margin-left:6px">chemin critique</span>' : ''}</div>
    </div>`;
  }

  // --- F13 : suivi du temps (▶ démarrer / ⏸ arrêter + cumul affiché)
  function fmtMinutes(total) {
    if (!total) return '0 min';
    const h = Math.floor(total / 60);
    const m = total % 60;
    return h ? `${h} h ${m ? `${String(m).padStart(2, '0')} min` : ''}` : `${m} min`;
  }
  function runningMinutes(card) {
    if (!card.tracking_started_at) return 0;
    const started = new Date(card.tracking_started_at.replace(' ', 'T'));
    if (isNaN(started)) return 0;
    return Math.max(0, Math.round((Date.now() - started.getTime()) / 60000));
  }
  function renderTracking(container, close) {
    const card = detail.card;
    const running = !!card.tracking_started_at;
    const total = (card.time_spent_minutes || 0) + runningMinutes(card);
    container.innerHTML = `
      <span class="badge" style="color:${running ? 'var(--success)' : 'var(--muted)'}">${running ? `● en cours (${fmtMinutes(runningMinutes(card))})` : '● à l\'arrêt'}</span>
      <b style="font-size:13px">${fmtMinutes(total)}</b>
      <button class="btn sm ${running ? '' : 'primary'}" id="cm-track-toggle">${running ? '⏸ Arrêter' : '▶ Démarrer'}</button>
      ${card.time_spent_minutes ? '<button class="btn sm ghost" id="cm-track-reset" title="Remettre le compteur à zéro">Réinitialiser</button>' : ''}`;
    container.querySelector('#cm-track-toggle').onclick = async () => {
      try {
        if (running) {
          const minutes = runningMinutes(card);
          detail.card = await api.updateCard(card.id, { time_spent_minutes: minutes, tracking_started_at: '' });
        } else {
          detail.card = await api.updateCard(card.id, { tracking_started_at: new Date().toISOString() });
        }
        notify();
        renderTracking(container, close);
      } catch (e) { toast(e.message, 'err'); }
    };
    const reset = container.querySelector('#cm-track-reset');
    if (reset) reset.onclick = async () => {
      try {
        if (running) await api.updateCard(card.id, { tracking_started_at: '' });
        detail.card = await api.updateCard(card.id, { time_spent_minutes: -(card.time_spent_minutes || 0) });
        notify();
        renderTracking(container, close);
      } catch (e) { toast(e.message, 'err'); }
    };
  }

  // --- F11 : activité de la carte (flux filtré par card_id)
  const ACTIVITY_LABELS = {
    card_created: 'Création',
    card_updated: 'Modification',
    card_moved: 'Déplacement',
    card_duplicated: 'Duplication',
    card_archived: 'Archivage',
    card_unarchived: 'Désarchivage',
    card_deleted: 'Suppression',
    card_labeled: 'Label ajouté',
    card_unlabeled: 'Label retiré',
    card_commented: 'Commentaire',
    checklist_item_checked: 'Case cochée',
    checklist_item_added: 'Élément de checklist',
    attachment_added: 'Pièce jointe',
    attachment_deleted: 'Pièce jointe retirée',
    cover_changed: 'Couverture',
    field_set: 'Champ personnalisé',
    recurrence_next: 'Récurrence',
    recurrence_error: 'Erreur de récurrence',
    tracking_started: 'Suivi démarré',
    tracking_stopped: 'Suivi arrêté',
    dependency_added: 'Dépendance ajoutée',
    dependency_removed: 'Dépendance retirée',
    baseline_created: 'Baseline créée',
  };
  function activityDetail(kind, payload) {
    let p; try { p = JSON.parse(payload || '{}'); } catch { return ''; }
    if (kind === 'card_moved') {
      const from = p.from_list_title || p.from_list_id || '';
      const to = p.to_list_title || p.to_list_id || '';
      return from && to ? `${from} → ${to}` : '';
    }
    if (kind === 'attachment_added') return p.filename || '';
    if (kind === 'recurrence_next') return p.due ? `prochaine échéance : ${p.due}` : '';
    if (kind === 'dependency_added') return p.predecessor_id ? `${p.predecessor_id} → ${p.successor_id}` : '';
    return '';
  }
  async function renderActivity(container) {
    try {
      const acts = await api.boardActivity(detail.card.board_id, 30, detail.card.id);
      container.innerHTML = acts.map((a) => {
        const label = ACTIVITY_LABELS[a.kind] || a.kind;
        const when = (a.created_at || '').replace('T', ' ').slice(0, 16);
        const detailTxt = activityDetail(a.kind, a.payload);
        return `<div class="act-row" style="display:flex;gap:8px;align-items:center;padding:3px 0;font-size:12px;border-bottom:1px solid var(--border)">
          <span class="badge" style="flex:none">${escapeHtml(label)}</span>
          <span class="muted" style="flex:none">${escapeHtml(when)}</span>
          <span style="overflow:hidden;text-overflow:ellipsis;white-space:nowrap">${escapeHtml(detailTxt)}</span>
        </div>`;
      }).join('') || '<span class="muted">Aucune activité pour cette carte.</span>';
    } catch (e) {
      container.innerHTML = `<span class="muted">${escapeHtml(e.message)}</span>`;
    }
  }

  function render() {
    const card = detail.card;
    const labelIds = (detail.labels || []).map((l) => l.id);
    const pct = progressOf(checklists);
    const currentCover = getCoverValue(card.cover);

    // Candidats comme carte parente : même liste, actives, hors soi et hors
    // propre descendance (anti-cycle).
    const sameListCandidates = (() => {
      const byId = new Map(state.cards.map((c) => [c.id, c]));
      const isDesc = (c) => {
        let p = c.parent_id;
        while (p) {
          if (p === card.id) return true;
          p = byId.get(p)?.parent_id || null;
        }
        return false;
      };
      return state.cards
        .filter((c) => c.list_id === card.list_id && !c.archived && !c.is_template && c.id !== card.id && !isDesc(c))
        .sort((a, b) => a.position - b.position);
    })();

    const { el, close } = openModal(`
      <div class="modal-head">
        <h2 data-edit="title" title="Cliquez pour modifier le titre" style="cursor:pointer">${escapeHtml(card.title)}</h2>
        <button class="iconbtn" id="cm-copy-link" title="Copier le lien de la carte">
          ${icon('copy', { size: 16 })}
        </button>
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

            <div class="modal-section" id="cm-checklists-section">
              <div class="label" style="display:flex;justify-content:space-between;align-items:center">
                <span>Checklists</span>
                <span id="cm-checklists-badge" class="badge" style="color:var(--success);${pct !== null ? '' : 'display:none'}">${pct ?? 0}%</span>
              </div>
              <div id="cm-checklists-track" class="progress-track" style="${pct !== null ? '' : 'display:none'}">
                <div id="cm-checklists-fill" class="progress-fill ${pct === 100 ? 'done' : ''}" style="width:${pct ?? 0}%"></div>
              </div>
              <div id="cm-checklists">
                ${renderChecklistItems(checklists)}
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
                ${renderComments(comments)}
              </div>
            </div>
          </div>

          <div class="side">
            <div class="modal-section">
              <div class="label">Emplacement</div>
              <div class="row" style="gap:8px">
                <select class="input" id="cm-move-board" style="flex:1" title="Board">
                  ${(state.boards?.length ? state.boards : [{ id: card.board_id, title: 'Board courant' }])
                    .map((b) => `<option value="${b.id}" ${b.id === card.board_id ? 'selected' : ''}>${escapeHtml(b.title)}</option>`)
                    .join('')}
                </select>
                <select class="input" id="cm-move-list" style="flex:1" title="Liste">
                  <option value="${card.list_id}" selected>${escapeHtml(visibleLists().find((l) => l.id === card.list_id)?.title || 'Liste courante')}</option>
                </select>
              </div>
              <div class="muted" style="font-size:11.5px;margin-top:6px">Déplacer vers un autre board : la carte voyage seule, ses labels et champs personnalisés (propres au board d'origine) ne suivent pas.</div>
            </div>

            <div class="modal-section">
              <div class="label">Modèle</div>
              ${card.is_template ? `
                <div class="field">
                  <label>Aide dans la galerie</label>
                  <textarea class="input" id="cm-template-desc" rows="2" style="resize:vertical" placeholder="Texte d'aide affiché sous le modèle…">${escapeHtml(card.template_description || '')}</textarea>
                </div>
                <button class="btn primary sm" id="cm-from-template" style="width:100%;justify-content:center;margin-top:8px">${icon('plus', { size: 13 })} <span>Créer une carte depuis ce modèle</span></button>
                <button class="btn ghost sm" id="cm-untoggle-template" style="margin-top:6px">Reconvertir en carte ordinaire</button>
              ` : `
                <button class="btn ghost sm" id="cm-make-template">${icon('template', { size: 13 })} <span>En faire un modèle</span></button>
                <div class="muted" style="font-size:11.5px;margin-top:6px">Un modèle sert de source : créez de nouvelles cartes en un clic depuis n'importe quelle liste.</div>
              `}
            </div>

            <div class="modal-section">
              <div class="label">Planification & Délais</div>
              ${card.is_template
                ? `<div class="muted" style="font-size:12px">${icon('alert', { size: 12 })} Dates et statut « terminée » masqués sur un modèle — jamais copiés vers les nouvelles cartes.</div>`
                : `
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
                <label>Carte parente</label>
                <select class="input" id="cm-parent" title="Rattacher cette carte comme sous-carte d'une carte de la même liste">
                  <option value="" ${card.parent_id ? '' : 'selected'}>— Aucune (top-level) —</option>
                  ${sameListCandidates.map((p) => `<option value="${p.id}" ${card.parent_id === p.id ? 'selected' : ''}>${escapeHtml(p.title)}</option>`).join('')}
                </select>
              </div>
              <div class="field">
                <label>Rappels</label>
                <div id="cm-reminders"></div>
                <button type="button" class="btn ghost sm" id="cm-add-reminder" style="margin-top:6px">＋ Ajouter un rappel</button>
              </div>
              `}
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

            <div class="modal-section">
              <div class="label">Temps passé</div>
              <div class="row" id="cm-tracking" style="gap:8px;align-items:center"></div>
            </div>

            <div class="modal-section">
              <div class="label">Activité</div>
              <div id="cm-activity"><span class="muted">…</span></div>
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
                ${renderAttachments(attachments)}
              </div>
            </div>

            <div class="modal-section">
              <div class="label">Ordonnancement CPM</div>
              <div id="cm-cpm-schedule">
                ${renderCpmSchedule(schedule)}
              </div>
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
      </div>`, { id: 'card-modal', size: 'lg', onClose: () => clearInterval(window.__cmTrackTimer) });

    // F13 : suivi du temps (temps en cours rafraîchi toutes les 30 s)
    renderTracking(el.querySelector('#cm-tracking'), close);
    if (window.__cmTrackTimer) clearInterval(window.__cmTrackTimer);
    window.__cmTrackTimer = setInterval(() => renderTracking(el.querySelector('#cm-tracking'), close), 30000);

    // F11 : activité de la carte
    renderActivity(el.querySelector('#cm-activity'));

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
    schedule = d.schedule;

    const el = current?.el;
    if (el && el.isConnected) {
      // Mise à jour ciblée dans le DOM existant : zéro flash, focus et scroll conservés !
      const titleEl = el.querySelector('[data-edit=title]');
      if (titleEl) titleEl.textContent = detail.card.title;

      const descEdit = el.querySelector('#cm-desc');
      const isDescEditing = descEdit && descEdit.style.display !== 'none';
      if (!isDescEditing) {
        const descView = el.querySelector('#cm-desc-view');
        if (descView) {
          descView.innerHTML = mdToHtml(detail.card.description) || '<span class="muted">Cliquez pour ajouter une description…</span>';
        }
      }

      const clHost = el.querySelector('#cm-checklists');
      if (clHost) clHost.innerHTML = renderChecklistItems(checklists);
      updateChecklistProgress(el, checklists);

      const comHost = el.querySelector('#cm-comments');
      if (comHost) comHost.innerHTML = renderComments(comments);

      const attHost = el.querySelector('#cm-attachments');
      if (attHost) attHost.innerHTML = renderAttachments(attachments);

      const depHost = el.querySelector('#cm-deps');
      if (depHost) depHost.innerHTML = renderDeps(detail);

      const cpmHost = el.querySelector('#cm-cpm-schedule');
      if (cpmHost) cpmHost.innerHTML = renderCpmSchedule(detail.schedule);

      const curCover = getCoverValue(detail.card.cover);
      el.querySelectorAll('#cm-covers [data-cover]').forEach((btn) => {
        btn.classList.toggle('sel', btn.dataset.cover === (curCover || ''));
      });

      const c = state.cards.find((c) => c.id === detail.card.id);
      if (c) {
        Object.assign(c, detail.card);
        c._checklists = checklists;
      }
      if (window.__krelloUpdateCardTile) window.__krelloUpdateCardTile(detail.card.id);
    } else {
      current = render();
    }
  }

  let current = render();

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

    // --- copier le lien profond de la carte
    el.querySelector('#cm-copy-link')?.addEventListener('click', async () => {
      const url = `${location.origin}${location.pathname}#/b/${card.board_id}/board/c/${card.id}`;
      try {
        await navigator.clipboard.writeText(url);
        toast('Lien de la carte copié', 'ok');
      } catch {
        toast('Copie impossible dans ce contexte', 'err');
      }
    });

    // --- titre inline
    const title = el.querySelector('[data-edit=title]');
    title.addEventListener('click', async () => {
      const nv = await promptDialog('Modifier le titre', card.title);
      if (nv && nv !== card.title) {
        card.title = nv;
        title.textContent = nv;
        try {
          detail.card = await api.updateCard(card.id, { title: nv });
          const c = state.cards.find((c) => c.id === card.id);
          if (c) c.title = nv;
          if (window.__krelloUpdateCardTile) window.__krelloUpdateCardTile(card.id);
        } catch (err) { toast(err.message, 'err'); }
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
    el.querySelector('#cm-desc-cancel').onclick = () => {
      descEdit.style.display = 'none';
      descActions.style.display = 'none';
      descView.style.display = '';
      descView.innerHTML = mdToHtml(card.description) || '<span class="muted">Cliquez pour ajouter une description…</span>';
    };
    el.querySelector('#cm-desc-save').onclick = async () => {
      const nv = descEdit.value;
      try {
        detail.card = await api.updateCard(card.id, { description: nv });
        card.description = nv;
        descEdit.style.display = 'none';
        descActions.style.display = 'none';
        descView.style.display = '';
        descView.innerHTML = mdToHtml(nv) || '<span class="muted">Cliquez pour ajouter une description…</span>';
        const c = state.cards.find((c) => c.id === card.id);
        if (c) c.description = nv;
      } catch (err) { toast(err.message, 'err'); }
    };

    // --- déplacement inter-listes / inter-boards
    const moveBoard = el.querySelector('#cm-move-board');
    const moveList = el.querySelector('#cm-move-list');
    const fillMoveLists = async (boardId) => {
      let lists = boardId === card.board_id
        ? state.lists
        : await api.lists(boardId).catch(() => []);
      lists = (lists || []).filter((l) => !l.archived);
      moveList.innerHTML = lists.map((l) =>
        `<option value="${l.id}" ${l.id === card.list_id ? 'selected' : ''}>${escapeHtml(l.title)}</option>`
      ).join('') || '<option value="">—</option>';
    };
    moveBoard?.addEventListener('change', () => fillMoveLists(moveBoard.value));

    // --- sous-carte : rattachement à une carte parente de la même liste
    el.querySelector('#cm-parent')?.addEventListener('change', async (e) => {
      try {
        await api.moveCard(card.id, { list_id: card.list_id, parent_id: e.target.value });
        toast(e.target.value ? 'Carte rattachée au parent' : 'Carte détachée (top-level)', 'ok');
        close();
      } catch (err) { toast(err.message, 'err'); }
    });
    moveList?.addEventListener('change', async () => {
      if (!moveList.value) return;
      try {
        await api.moveCard(card.id, { list_id: moveList.value });
        toast('Carte déplacée', 'ok');
        close();
      } catch (err) { toast(err.message, 'err'); }
    });

    // --- attributs
    const upd = (patch) => api.updateCard(card.id, patch).then((c) => {
      detail.card = c;
      const cardInState = state.cards.find((x) => x.id === card.id);
      if (cardInState) Object.assign(cardInState, c);
      if (window.__krelloUpdateCardTile) window.__krelloUpdateCardTile(card.id);
    }).catch((e) => toast(e.message, 'err'));

    el.querySelector('#cm-due')?.addEventListener('change', (e) => {
      const time = el.querySelector('#cm-due-time')?.value;
      upd({ due: e.target.value ? (e.target.value + (time ? `T${time}:00` : '')) : null });
    });
    el.querySelector('#cm-due-time')?.addEventListener('change', (e) => {
      const d = el.querySelector('#cm-due')?.value;
      if (d) upd({ due: d + (e.target.value ? `T${e.target.value}:00` : '') });
    });
    el.querySelector('#cm-due-complete')?.addEventListener('change', (e) => upd({ due_complete: e.target.checked }));
    el.querySelector('#cm-start')?.addEventListener('change', (e) => upd({ start: e.target.value || null }));
    // --- rappels multiples (relatifs à l'échéance ou date fixe)
    const OFFSETS = [['15m', '15 min avant'], ['30m', '30 min avant'], ['1h', '1 h avant'], ['2h', '2 h avant'], ['1d', '1 jour avant'], ['2d', '2 jours avant'], ['1w', '1 semaine avant']];
    let reminders = Array.isArray(card.reminders) ? [...card.reminders] : [];
    const remWrap = el.querySelector('#cm-reminders');
    const renderReminders = () => {
      remWrap.innerHTML = reminders.length ? reminders.map((r, i) => `
        <div style="display:flex;gap:6px;align-items:center;margin-bottom:4px">
          <select class="input" data-rkind="${i}" style="flex:0 0 118px" title="Type de rappel">
            <option value="relative" ${r.kind === 'relative' ? 'selected' : ''}>Avant l'échéance</option>
            <option value="absolute" ${r.kind === 'absolute' ? 'selected' : ''}>Date fixe</option>
          </select>
          ${r.kind === 'absolute'
            ? `<input type="datetime-local" class="input" data-rat="${i}" value="${String(r.at || '').slice(0, 16)}" style="flex:1" title="Date et heure du rappel">`
            : `<select class="input" data-roffset="${i}" style="flex:1" title="Délai avant l'échéance">
                ${OFFSETS.map(([v, lbl]) => `<option value="${v}" ${r.offset === v ? 'selected' : ''}>${lbl}</option>`).join('')}
              </select>`}
          <button type="button" class="iconbtn" data-rdel="${i}" title="Supprimer ce rappel" style="flex:none">✕</button>
        </div>`).join('')
        : `<div class="muted" style="font-size:12.5px">Aucun rappel</div>`;
    };
    const saveReminders = () => {
      renderReminders();
      upd({ reminders });
    };
    renderReminders();
    remWrap.addEventListener('change', (e) => {
      const t = e.target;
      const i = Number(t.dataset.rkind ?? t.dataset.roffset ?? t.dataset.rat);
      if (Number.isNaN(i) || !reminders[i]) return;
      if (t.dataset.rkind !== undefined) {
        reminders[i] = t.value === 'absolute'
          ? { kind: 'absolute', at: `${new Date().toISOString().slice(0, 10)}T09:00` }
          : { kind: 'relative', offset: '15m' };
      } else if (t.dataset.roffset !== undefined) {
        reminders[i] = { kind: 'relative', offset: t.value };
      } else if (t.dataset.rat !== undefined) {
        const v = t.value; // "YYYY-MM-DDTHH:MM"
        reminders[i] = { kind: 'absolute', at: v.length === 16 ? `${v}:00` : v };
      }
      saveReminders();
    });
    remWrap.addEventListener('click', (e) => {
      const btn = e.target.closest('[data-rdel]');
      if (!btn) return;
      reminders.splice(Number(btn.dataset.rdel), 1);
      saveReminders();
    });
    el.querySelector('#cm-add-reminder')?.addEventListener('click', () => {
      reminders.push({ kind: 'relative', offset: '15m' });
      saveReminders();
    });
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
        detail.labels = ids.map((id) => labelById(id)).filter(Boolean);
        const cardInState = state.cards.find((x) => x.id === card.id);
        if (cardInState) cardInState._label_ids = ids;
        if (window.__krelloUpdateCardTile) window.__krelloUpdateCardTile(card.id);
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
      try {
        await api.setCover(card.id, c || null);
        detail.card.cover = c ? JSON.stringify({ type: 'color', value: c }) : null;
        card.cover = detail.card.cover;
        el.querySelectorAll('#cm-covers [data-cover]').forEach((btn) => {
          btn.classList.toggle('sel', btn.dataset.cover === (c || ''));
        });
        const cardInState = state.cards.find((x) => x.id === card.id);
        if (cardInState) cardInState.cover = card.cover;
        if (window.__krelloUpdateCardTile) window.__krelloUpdateCardTile(card.id);
      } catch (err) { toast(err.message, 'err'); }
    });

    // --- checklists : mise à jour ciblée et instantanée (aucun flash !)
    el.querySelector('#cm-add-checklist').onclick = async () => {
      const title = await promptDialog('Titre de la checklist', 'Étapes');
      if (!title) return;
      try {
        const cl = await api.createChecklist(card.id, { title });
        cl.items = [];
        checklists.push(cl);
        el.querySelector('#cm-checklists').innerHTML = renderChecklistItems(checklists);
        updateChecklistProgress(el, checklists);
        const c = state.cards.find((c) => c.id === card.id);
        if (c) c._checklists = checklists;
        if (window.__krelloUpdateCardTile) window.__krelloUpdateCardTile(card.id);
      } catch (err) { toast(err.message, 'err'); }
    };

    el.querySelector('#cm-checklists').addEventListener('click', async (e) => {
      const addBtn = e.target.closest('[data-add-item]');
      if (addBtn) {
        const t = await promptDialog('Nouvel élément');
        if (t) {
          try {
            const it = await api.createChecklistItem(addBtn.dataset.addItem, { title: t });
            const cl = checklists.find((c) => c.id === addBtn.dataset.addItem);
            if (cl) {
              if (!cl.items) cl.items = [];
              cl.items.push(it);
            }
            el.querySelector('#cm-checklists').innerHTML = renderChecklistItems(checklists);
            updateChecklistProgress(el, checklists);
            const c = state.cards.find((c) => c.id === card.id);
            if (c) c._checklists = checklists;
            if (window.__krelloUpdateCardTile) window.__krelloUpdateCardTile(card.id);
          } catch (err) { toast(err.message, 'err'); }
        }
        return;
      }

      const delItem = e.target.closest('[data-del-item]');
      if (delItem) {
        const itemId = delItem.dataset.delItem;
        try {
          await api.deleteChecklistItem(itemId);
          for (const cl of checklists) {
            cl.items = (cl.items || []).filter((it) => it.id !== itemId);
          }
          el.querySelector('#cm-checklists').innerHTML = renderChecklistItems(checklists);
          updateChecklistProgress(el, checklists);
          const c = state.cards.find((c) => c.id === card.id);
          if (c) c._checklists = checklists;
          if (window.__krelloUpdateCardTile) window.__krelloUpdateCardTile(card.id);
        } catch (err) { toast(err.message, 'err'); }
        return;
      }

      const delCl = e.target.closest('[data-del-cl]');
      if (delCl) {
        if (await confirmDialog('Supprimer cette checklist ?')) {
          const clId = delCl.dataset.delCl;
          try {
            await api.deleteChecklist(clId);
            checklists = checklists.filter((cl) => cl.id !== clId);
            el.querySelector('#cm-checklists').innerHTML = renderChecklistItems(checklists);
            updateChecklistProgress(el, checklists);
            const c = state.cards.find((c) => c.id === card.id);
            if (c) c._checklists = checklists;
            if (window.__krelloUpdateCardTile) window.__krelloUpdateCardTile(card.id);
          } catch (err) { toast(err.message, 'err'); }
        }
      }
    });

    el.querySelector('#cm-checklists').addEventListener('change', async (e) => {
      const cb = e.target.closest('[data-item]');
      if (!cb) return;
      const itemId = cb.dataset.item;
      const isChecked = cb.checked;

      // 1. Visuel immédiat et optimiste
      const line = cb.closest('.checkline');
      const textSpan = line?.querySelector('.t');
      if (textSpan) textSpan.classList.toggle('done', isChecked);

      // 2. État local
      for (const cl of checklists) {
        const it = (cl.items || []).find((i) => i.id === itemId);
        if (it) {
          it.checked = isChecked;
          break;
        }
      }

      // 3. Animation spring fluide de la barre de progression (aucun flash !)
      updateChecklistProgress(el, checklists);

      // 4. Synchronisation immédiate de la tuile sur le board
      const c = state.cards.find((c) => c.id === card.id);
      if (c) c._checklists = checklists;
      if (window.__krelloUpdateCardTile) window.__krelloUpdateCardTile(card.id);

      // 5. Appel réseau en arrière-plan sans recharger la modale ni la fenêtre
      try {
        await api.updateChecklistItem(itemId, { checked: isChecked });
      } catch (err) {
        // Rollback propre si l'appel échoue
        cb.checked = !isChecked;
        if (textSpan) textSpan.classList.toggle('done', !isChecked);
        for (const cl of checklists) {
          const it = (cl.items || []).find((i) => i.id === itemId);
          if (it) { it.checked = !isChecked; break; }
        }
        updateChecklistProgress(el, checklists);
        if (c) c._checklists = checklists;
        if (window.__krelloUpdateCardTile) window.__krelloUpdateCardTile(card.id);
        toast(`Erreur : ${err.message}`, 'err');
      }
    });

    // --- commentaires
    const addComment = async () => {
      const input = el.querySelector('#cm-comment');
      const body = input.value.trim();
      if (!body) return;
      input.value = '';
      try {
        const nc = await api.addComment(card.id, body);
        comments.unshift(nc);
        el.querySelector('#cm-comments').innerHTML = renderComments(comments);
        const c = state.cards.find((c) => c.id === card.id);
        if (c) c._comments_count = comments.length;
        if (window.__krelloUpdateCardTile) window.__krelloUpdateCardTile(card.id);
      } catch (err) { toast(err.message, 'err'); }
    };
    el.querySelector('#cm-comment-add').onclick = addComment;
    el.querySelector('#cm-comment').addEventListener('keydown', (e) => { if (e.key === 'Enter') addComment(); });
    el.querySelector('#cm-comments').addEventListener('click', async (e) => {
      const del = e.target.closest('[data-del-com]');
      if (del) {
        const cid = del.dataset.delCom;
        try {
          await api.deleteComment(cid);
          comments = comments.filter((c) => c.id !== cid);
          el.querySelector('#cm-comments').innerHTML = renderComments(comments);
          const c = state.cards.find((c) => c.id === card.id);
          if (c) c._comments_count = comments.length;
          if (window.__krelloUpdateCardTile) window.__krelloUpdateCardTile(card.id);
        } catch (err) { toast(err.message, 'err'); }
      }
    });

    // --- pièces jointes
    el.querySelector('#cm-file').addEventListener('change', async (e) => {
      const file = e.target.files[0];
      if (!file) return;
      try {
        const att = await api.uploadAttachment(card.id, file);
        attachments.push(att);
        el.querySelector('#cm-attachments').innerHTML = renderAttachments(attachments);
        const c = state.cards.find((c) => c.id === card.id);
        if (c) c._attachments_count = attachments.length;
        if (window.__krelloUpdateCardTile) window.__krelloUpdateCardTile(card.id);
      } catch (err) { toast(err.message, 'err'); }
    });
    el.querySelector('#cm-attachments').addEventListener('click', async (e) => {
      const del = e.target.closest('[data-del-att]');
      if (del) {
        const aid = del.dataset.delAtt;
        try {
          await api.deleteAttachment(aid);
          attachments = attachments.filter((a) => a.id !== aid);
          el.querySelector('#cm-attachments').innerHTML = renderAttachments(attachments);
          const c = state.cards.find((c) => c.id === card.id);
          if (c) c._attachments_count = attachments.length;
          if (window.__krelloUpdateCardTile) window.__krelloUpdateCardTile(card.id);
        } catch (err) { toast(err.message, 'err'); }
        return;
      }
      const cov = e.target.closest('[data-cover-att]');
      if (cov) {
        try {
          await api.setAttachmentCover(cov.dataset.coverAtt);
          refreshData();
        } catch (err) { toast(err.message, 'err'); }
      }
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
      if (del) {
        try {
          await api.deleteDep(del.dataset.delDep);
          refreshData();
        } catch (err) { toast(err.message, 'err'); }
      }
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

    // --- modèle
    el.querySelector('#cm-make-template')?.addEventListener('click', async () => {
      if ((card.start || card.due) && !(await confirmDialog('Cette carte contient des dates : elles resteront sur le modèle mais ne seront jamais copiées vers les nouvelles cartes. Continuer ?'))) return;
      try {
        detail.card = await api.setCardTemplate(card.id, { is_template: true });
        refreshData();
      } catch (e) { toast(e.message, 'err'); }
    });
    el.querySelector('#cm-untoggle-template')?.addEventListener('click', async () => {
      try {
        detail.card = await api.setCardTemplate(card.id, { is_template: false });
        refreshData();
      } catch (e) { toast(e.message, 'err'); }
    });
    el.querySelector('#cm-template-desc')?.addEventListener('change', async (e) => {
      try {
        detail.card = await api.setCardTemplate(card.id, { is_template: true, template_description: e.target.value });
        notify();
      } catch (err) { toast(err.message, 'err'); }
    });
    el.querySelector('#cm-from-template')?.addEventListener('click', () => {
      const fm = openModal(`
        <div class="modal-head">
          <h2>Créer une carte depuis ce modèle</h2>
          <button class="iconbtn" data-close title="Fermer">${icon('close', { size: 16 })}</button>
        </div>
        <div class="modal-body">
          <div class="field"><label>Liste de destination</label>
            <select class="input" id="ft-list">${visibleLists().map((l) => `<option value="${l.id}" ${l.id === card.list_id ? 'selected' : ''}>${escapeHtml(l.title)}</option>`).join('')}</select>
          </div>
          <div class="field" style="margin-top:8px"><label>Titre de la carte</label>
            <input class="input" id="ft-title" value="${escapeHtml(card.title)}" />
          </div>
          <div class="row" style="justify-content:flex-end;margin-top:14px">
            <button class="btn primary" id="ft-create">${icon('plus', { size: 14 })} <span>Créer la carte</span></button>
          </div>
        </div>`, { id: 'from-template-modal' });
      const titleInput = fm.el.querySelector('#ft-title');
      titleInput.focus();
      titleInput.select();
      const create = async () => {
        try {
          const nc = await api.instantiateTemplate(card.id, {
            list_id: fm.el.querySelector('#ft-list').value,
            name: titleInput.value || undefined,
            at: 'bottom',
          });
          fm.close();
          close();
          notify();
          openCardModal(nc.id, { onChange });
        } catch (err) { toast(err.message, 'err'); }
      };
      fm.el.querySelector('#ft-create').onclick = create;
      titleInput.addEventListener('keydown', (e) => { if (e.key === 'Enter') create(); });
    });

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
