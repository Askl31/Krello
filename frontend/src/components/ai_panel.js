// Tiroir IA (topbar ✨) : actions rapides Apple Intelligence sur le board courant.

import { api } from '../api.js';
import { state, loadBoard } from '../state.js';
import { toast } from '../ui/modal.js';
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
      const input = prompt('Décrivez la tâche en une phrase :', 'Préparer la revue de sprint vendredi prochain, priorité haute');
      if (!input) return;
      out.textContent = 'Génération…';
      const r = await api.aiParseTask(input, firstList.id, true);
      out.innerHTML = `✅ Carte créée : <b>${r.card.title}</b>${r.parsed.due ? ` · échéance ${r.parsed.due}` : ''}`;
      await loadBoard(board.id);
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
