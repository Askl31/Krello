// Tiroir IA (topbar ✨) : actions rapides Apple Intelligence sur le board courant.

import { api } from '../api.js';
import { state, loadBoard } from '../state.js';
import { toast, openModal } from '../ui/modal.js';
import { escapeHtml } from '../format.js';
import { on } from '../ws.js';
import { icon } from '../icons.js';

let drawer = null;

export async function initAiDrawer() {
  if (drawer) {
    drawer.remove();
    drawer = null;
    return;
  }
  let status;
  try { status = await api.aiStatus(); } catch { status = { available: false }; }
  if (!status.available) {
    toast(`Assistant IA indisponible (${status.reason || 'raison inconnue'})`, 'err');
    return;
  }
  state.aiStatus = status;
  drawer = document.createElement('div');
  drawer.className = 'ai-drawer';
  drawer.innerHTML = `
    <div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:12px;padding-bottom:8px;border-bottom:1px solid var(--border)">
      <div style="display:flex;align-items:center;gap:8px">
        <span style="color:var(--accent)">${icon('ai', { size: 16 })}</span>
        <b style="font-size:13.5px">Apple Intelligence (on-device)</b>
      </div>
      <button class="iconbtn" id="ai-drawer-close" style="width:24px;height:24px">${icon('close', { size: 13 })}</button>
    </div>
    <div class="ai-row">
      <button class="btn sm" data-act="nl">${icon('plus', { size: 13 })} <span>Carte en langage naturel</span></button>
      <button class="btn sm" data-act="plan">${icon('timeline', { size: 13 })} <span>Planifier un projet</span></button>
      <button class="btn sm" data-act="prio">${icon('arrowUp', { size: 13 })} <span>Suggérer priorités</span></button>
      <button class="btn sm" data-act="sum">${icon('dashboard', { size: 13 })} <span>Résumer le board</span></button>
    </div>
    <div id="ai-out" class="muted" style="font-size:13px;padding:8px;background:var(--surface-2);border-radius:var(--radius-sm);margin-top:6px">
      Choisissez une action pour commencer…
    </div>`;

  document.querySelector('.main').style.position = 'relative';
  document.querySelector('.main').appendChild(drawer);
  drawer.addEventListener('click', onAction);
  drawer.querySelector('#ai-drawer-close').onclick = () => { drawer?.remove(); drawer = null; };

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
      out.textContent = 'Planification en cours par Apple Intelligence…';
      const r = await api.aiPlan(goal, board.id, firstList.id, true);
      out.innerHTML = `✅ <b>${r.cards.length}</b> tâches créées, <b>${r.dependencies.length}</b> dépendances CPM.<br/>Fin estimée : <b>${escapeHtml(r.schedule.finish || '')}</b>`;
      await loadBoard(board.id);
    } else if (act === 'prio') {
      out.textContent = 'Analyse des priorités en cours…';
      const r = await api.aiPriorities(board.id, false);
      out.innerHTML = `<b>Suggestions d'optimisation :</b><br>` + r.suggestions
        .map((s) => `• <b>${escapeHtml((s.priority || '').toUpperCase())}</b> : ${escapeHtml(s.reason)}`).join('<br>')
        + `<div style="margin-top:10px"><button class="btn sm primary" data-act="prio-apply">Appliquer à tout le board</button></div>`;
    } else if (act === 'prio-apply') {
      const r = await api.aiPriorities(board.id, true);
      toast(`${r.suggestions.length} priorités appliquées avec succès`, 'ok');
      await loadBoard(board.id);
    } else if (act === 'sum') {
      out.textContent = 'Génération de la synthèse…';
      const r = await api.aiSummarize(board.id);
      out.innerHTML = `<pre style="font-size:12.5px;line-height:1.5">${escapeHtml(r.summary)}</pre>`;
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
    toast('Ouvrez un board avec au moins une liste active', 'err');
    return;
  }
  const { el, close } = openModal(`
    <div class="dialog-header">
      <div class="dialog-badge" style="background:linear-gradient(135deg,rgba(99,102,241,0.2),rgba(236,72,153,0.2));color:var(--accent);border-color:color-mix(in srgb,var(--accent)30%,transparent)">
        ${icon('ai', { size: 18 })}
      </div>
      <div class="dialog-titles">
        <h3 class="dialog-title">Nouvelle carte en langage naturel</h3>
        <p class="dialog-desc">Apple Intelligence extrait automatiquement le titre, l'échéance, la priorité et les sous-tâches.</p>
      </div>
      <button class="iconbtn dialog-close" data-close title="Fermer (Échap)">${icon('close', { size: 14 })}</button>
    </div>
    <div class="dialog-body">
      <div class="field" style="margin-bottom:12px">
        <label>Description de votre tâche</label>
        <textarea class="input" id="nl-input" rows="2"
          placeholder="Ex. : Préparer la revue de sprint pour vendredi prochain, priorité haute avec 3 étapes"></textarea>
      </div>
      <div class="field" style="margin-bottom:8px">
        <label>Liste de destination</label>
        <select class="input" id="nl-list">
          ${state.lists.filter((l) => !l.archived)
            .map((l) => `<option value="${l.id}">${escapeHtml(l.title)}</option>`).join('')}
        </select>
      </div>
      <div id="nl-result" class="muted" style="font-size:12.5px;min-height:20px;padding:2px 0"></div>
    </div>
    <div class="dialog-footer">
      <div class="dialog-hints">
        <span class="kbd">↵ Entrée</span> pour créer · <span class="kbd">Échap</span>
      </div>
      <div class="dialog-btns">
        <button class="btn" data-close>Annuler</button>
        <button class="btn primary" id="nl-create">Créer la carte</button>
      </div>
    </div>`, { size: 'md' });

  const input = el.querySelector('#nl-input');
  const result = el.querySelector('#nl-result');
  const btn = el.querySelector('#nl-create');
  input.focus();

  const create = async () => {
    const text = input.value.trim();
    if (!text) return;
    btn.disabled = true;
    btn.textContent = 'Analyse par Apple Intelligence…';
    try {
      const r = await api.aiParseTask(text, el.querySelector('#nl-list').value, true);
      const p = r.parsed || {};
      result.innerHTML = `<span style="color:var(--success)">✓</span> Carte créée : <b>${escapeHtml(p.title || '?')}</b>`
        + (p.due ? ` · ⏰ ${escapeHtml(p.due)}` : '')
        + (p.priority && p.priority !== 'none' ? ` · ▲ ${escapeHtml(p.priority)}` : '')
        + (p.labels?.length ? ` · ${p.labels.map((l) => escapeHtml(l)).join(', ')}` : '')
        + (p.checklist?.length ? ` · ☑ ${p.checklist.length} étapes` : '');
      input.value = '';
      input.focus();
    } catch (e) {
      result.textContent = `Erreur : ${e.message}`;
    } finally {
      btn.disabled = false;
      btn.textContent = 'Créer la carte (Entrée ↵)';
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
