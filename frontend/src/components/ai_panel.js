// Tiroir IA (topbar ✨) : actions rapides Apple Intelligence sur le board courant.

import { api } from '../api.js';
import { state, loadBoard } from '../state.js';
import { toast, openModal } from '../ui/modal.js';
import { escapeHtml } from '../format.js';
import { on } from '../ws.js';

let drawer = null;

export async function initAiDrawer() {
  if (drawer) { drawer.remove(); drawer = null; return; }
  let status;
  try { status = await api.aiStatus(); } catch { status = { available: false }; }
  if (!status.available) {
    toast(`Assistant IA indisponible (${status.reason || 'raison inconnue'})`, 'err');
    return;
  }
  state.aiStatus = status;
  drawer = document.createElement('div');
  drawer.className = 'ai-drawer fade-in';
  drawer.innerHTML = `
    <div class="ai-row">
      <button class="btn sm" data-act="nl">＋ Carte en langage naturel</button>
      <button class="btn sm" data-act="plan">🗓 Planifier un projet</button>
      <button class="btn sm" data-act="prio">▲ Suggérer priorités</button>
      <button class="btn sm" data-act="sum">📝 Résumer le board</button>
    </div>
    <div id="ai-out" class="muted" style="font-size:13px">Choisissez une action…</div>`;
  document.querySelector('.main').style.position = 'relative';
  document.querySelector('.main').appendChild(drawer);
  drawer.addEventListener('click', onAction);
  on('board.loaded', () => { if (drawer && !drawer.isConnected) drawer = null; });
}

async function onAction(e) {
  const act = e.target.closest('[data-act]')?.dataset.act;
  if (!act) return;
  const out = drawer.querySelector('#ai-out');
  const board = state.board;
  const firstList = state.lists[0];
  try {
    if (act === 'nl') {
      openNaturalCardDialog();
      return;
    } else if (act === 'plan') {
      const goal = prompt('Objectif du projet :', 'Organiser un lancement produit en 3 semaines');
      if (!goal) return;
      out.textContent = 'Planification…';
      const r = await api.aiPlan(goal, board.id, firstList.id, true);
      out.innerHTML = `✅ ${r.cards.length} tâches créées, ${r.dependencies.length} dépendances, fin projet : <b>${r.schedule.finish}</b>`;
      await loadBoard(board.id);
    } else if (act === 'prio') {
      out.textContent = 'Analyse des priorités…';
      const r = await api.aiPriorities(board.id, false);
      out.innerHTML = `<b>Suggestions :</b><br>` + r.suggestions
        .map((s) => `• ${s.priority.toUpperCase()} — ${s.card_id.slice(-6)} <span class="muted">${s.reason}</span>`).join('<br>')
        + `<br><br><button class="btn sm primary" data-act="prio-apply">Appliquer</button>`;
    } else if (act === 'prio-apply') {
      const r = await api.aiPriorities(board.id, true);
      toast(`${r.suggestions.length} priorités appliquées`, 'ok');
      await loadBoard(board.id);
    } else if (act === 'sum') {
      out.textContent = 'Résumé…';
      const r = await api.aiSummarize(board.id);
      out.innerHTML = `<pre>${r.summary}</pre>`;
    }
  } catch (err) {
    out.textContent = `Erreur : ${err.message}`;
    toast(err.message, 'err');
  }
}

/**
 * Dialog « carte en langage naturel » (raccourci N, tiroir ✨).
 * Gating : IA disponible + board ouvert avec au moins une liste.
 */
export function openNaturalCardDialog() {
  if (!state.aiStatus?.available) {
    toast('IA indisponible sur cette machine', 'err');
    return;
  }
  if (!state.board || !state.lists.filter((l) => !l.archived).length) {
    toast('Ouvrez un board avec au moins une liste', 'err');
    return;
  }
  const { el, close } = openModal(`
    <div class="modal-head"><h2>✨ Nouvelle carte en langage naturel</h2>
      <button class="iconbtn" data-close title="Fermer">✕</button></div>
    <div class="modal-body">
      <div class="field"><label>Décrivez la tâche</label>
        <textarea class="input" id="nl-input" rows="2"
          placeholder="Ex. : préparer la revue de sprint vendredi prochain, priorité haute"></textarea>
      </div>
      <div class="field"><label>Liste de destination</label>
        <select class="input" id="nl-list">
          ${state.lists.filter((l) => !l.archived)
            .map((l) => `<option value="${l.id}">${escapeHtml(l.title)}</option>`).join('')}
        </select>
      </div>
      <div id="nl-result" class="muted" style="font-size:13px;min-height:18px"></div>
      <div class="row" style="justify-content:flex-end;margin-top:10px">
        <button class="btn" data-close>Fermer</button>
        <button class="btn primary" id="nl-create">Créer (Entrée)</button>
      </div>
    </div>`);

  const input = el.querySelector('#nl-input');
  const result = el.querySelector('#nl-result');
  const btn = el.querySelector('#nl-create');
  input.focus();

  const create = async () => {
    const text = input.value.trim();
    if (!text) return;
    btn.disabled = true;
    btn.textContent = 'Génération…';
    try {
      const r = await api.aiParseTask(text, el.querySelector('#nl-list').value, true);
      const p = r.parsed || {};
      result.innerHTML = `✅ <b>${escapeHtml(p.title || '?')}</b>`
        + (p.due ? ` · ⏰ ${escapeHtml(p.due)}` : '')
        + (p.priority && p.priority !== 'none' ? ` · ▲ ${escapeHtml(p.priority)}` : '')
        + (p.labels?.length ? ` · ${p.labels.map((l) => escapeHtml(l)).join(', ')}` : '')
        + (p.checklist?.length ? ` · ☑ ${p.checklist.length} étapes` : '');
      input.value = '';
      input.focus();
      // le board se met à jour tout seul via l'événement WS CardCreated
    } catch (e) {
      result.textContent = `Erreur : ${e.message}`;
    } finally {
      btn.disabled = false;
      btn.textContent = 'Créer (Entrée)';
    }
  };

  btn.addEventListener('click', create);
  input.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      create();
    }
  });
}
