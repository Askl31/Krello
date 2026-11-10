// Vue Dashboard : statistiques, graphiques SVG, baselines, résumé IA.

import { api } from '../api.js';
import { escapeHtml, fmtDateShort, parseDate, dueState } from '../format.js';
import { state, visibleLists } from '../state.js';
import { toast } from '../ui/modal.js';
import { icon } from '../icons.js';

export function renderDashboard() {
  const host = document.querySelector('.view');
  const cards = state.cards.filter((c) => !c.archived);
  const lists = visibleLists();
  const today = new Date();

  const overdue = cards.filter((c) => dueState(c, today) === 'overdue');
  const dueSoon = cards.filter((c) => dueState(c, today) === 'due-soon');
  const done = cards.filter((c) => c.due_complete || c.pct_complete === 100);
  const milestones = cards.filter((c) => c.is_milestone && !c.due_complete)
    .sort((a, b) => (a.due || '').localeCompare(b.due || ''));
  const avgPct = cards.length ? Math.round(cards.reduce((s, c) => s + c.pct_complete, 0) / cards.length) : 0;

  // charge par liste
  const perList = lists.map((l) => ({
    label: l.title,
    count: cards.filter((c) => c.list_id === l.id).length,
    wip: l.wip_limit,
  }));
  const maxList = Math.max(1, ...perList.map((p) => p.count));

  // par priorité
  const prioOrder = ['urgent', 'high', 'medium', 'low', 'none'];
  const prioColors = { urgent: 'var(--critical)', high: 'var(--error)', medium: 'var(--c3)', low: 'var(--c4)', none: 'var(--muted)' };
  const perPriority = prioOrder.map((p) => ({ p, count: cards.filter((c) => c.priority === p).length }));
  const maxPrio = Math.max(1, ...perPriority.map((p) => p.count));

  host.innerHTML = `
    <div class="stats">
      <div class="stat"><div class="v">${cards.length}</div><div class="k">Cartes actives</div></div>
      <div class="stat ${overdue.length ? 'err' : ''}"><div class="v">${overdue.length}</div><div class="k">En retard</div></div>
      <div class="stat ${dueSoon.length ? 'warn' : ''}"><div class="v">${dueSoon.length}</div><div class="k">Échéance ≤ 2 j</div></div>
      <div class="stat"><div class="v">${done.length}</div><div class="k">Tâches terminées</div></div>
      <div class="stat"><div class="v">${avgPct}%</div><div class="k">Avancement moyen</div></div>
    </div>

    <div class="charts">
      <div class="panel">
        <h3>Répartition par liste</h3>
        ${perList.map((p) => `
          <div class="hbar">
            <span class="lbl">${escapeHtml(p.label)}${p.wip ? ` <span class="muted">(WIP ${p.wip})</span>` : ''}</span>
            <div class="bar" style="width:${Math.round((p.count / maxList) * 100)}%; background:${p.wip && p.count > p.wip ? 'var(--warning)' : 'var(--c1)'}"></div>
            <b style="font-size:12px">${p.count}</b>
          </div>`).join('') || '<span class="muted">Aucune liste</span>'}
      </div>

      <div class="panel">
        <h3>Distribution des priorités</h3>
        ${perPriority.filter((p) => p.count).map((p) => `
          <div class="hbar">
            <span class="lbl" style="color:${prioColors[p.p]}">${p.p}</span>
            <div class="bar" style="width:${Math.round((p.count / maxPrio) * 100)}%;background:${prioColors[p.p]}"></div>
            <b style="font-size:12px">${p.count}</b>
          </div>`).join('') || '<span class="muted">Aucune priorité définie</span>'}
        ${overdue.length ? `
          <h3 style="margin-top:18px">Alertes de retard</h3>
          ${overdue.slice(0, 6).map((c) => `<div class="hbar"><span class="lbl" style="color:var(--error)">⚠ ${escapeHtml(c.title)}</span><span class="badge overdue">${fmtDateShort(c.due)}</span></div>`).join('')}` : ''}
      </div>

      <div class="panel">
        <h3>Jalons à venir</h3>
        ${milestones.slice(0, 8).map((m) => `
          <div class="hbar">
            <span class="lbl" style="display:flex;align-items:center;gap:5px">
              <span style="color:var(--warning)">◆</span>
              <span>${escapeHtml(m.title)}</span>
            </span>
            <span class="badge ${m.due ? '' : 'muted'}">${m.due ? fmtDateShort(m.due) : 'sans date'}</span>
          </div>`).join('') || '<span class="muted">Aucun jalon défini</span>'}
      </div>

      <div class="panel">
        <h3>Baselines (Instantanés Projet)</h3>
        <div id="dash-baselines"><span class="muted">Chargement des baselines…</span></div>
        <div class="row" style="margin-top:12px">
          <button class="btn sm" id="dash-new-baseline">
            ${icon('plus', { size: 12 })} <span>Créer une baseline</span>
          </button>
        </div>
      </div>
    </div>

    <div class="panel ai-panel">
      <div class="ai-head">
        <div class="ai-orb">${icon('ai', { size: 15 })}</div>
        <h3 style="margin:0">Résumé IA Apple Intelligence</h3>
        <div style="flex:1"></div>
        ${state.aiStatus?.available
          ? `<button class="btn sm primary" id="dash-ai-sum">${icon('ai', { size: 13 })} <span>Générer</span></button>`
          : '<span class="badge muted">IA indisponible sur cette machine</span>'}
      </div>
      <div id="dash-ai-out" class="muted" style="font-size:13px">${state.aiStatus?.available
        ? 'Apple Intelligence résume l\'avancement, les risques et les échéances critiques on-device.'
        : 'Apple Intelligence requiert un Mac Apple Silicon avec l\'option activée (Réglages › Apple Intelligence &amp; Siri) et macOS 15.1+.'}</div>
    </div>`;

  // baselines
  const blHost = host.querySelector('#dash-baselines');
  api.baselines(state.board.id).then(async (bases) => {
    if (!bases.length) { blHost.innerHTML = '<span class="muted">Aucune baseline — créez un instantané pour mesurer les dérives de planning.</span>'; return; }
    const rows = await Promise.all(bases.map(async (b) => {
      let late = 0, total = 0;
      try {
        const diffs = await api.compareBaseline(b.id);
        total = diffs.filter((d) => d.due_baseline).length;
        late = diffs.filter((d) => (d.delta_due_days || 0) > 0).length;
      } catch { /* ignore */ }
      return `<tr>
        <td style="font-weight:550">${escapeHtml(b.name)}</td>
        <td class="muted">${fmtDateShort(b.created_at)}</td>
        <td><span class="badge ${late > 0 ? 'due-soon' : 'done'}">${late}/${total} en dérive</span></td>
        <td style="text-align:right">
          <button class="iconbtn" data-del-bl="${b.id}" title="Supprimer" style="width:22px;height:22px">
            ${icon('trash', { size: 12 })}
          </button>
        </td>
      </tr>`;
    }));
    blHost.innerHTML = `<table style="width:100%;border-collapse:collapse;font-size:12.5px"><thead><tr style="border-bottom:1px solid var(--border)"><th style="text-align:left;padding:4px 6px">Nom</th><th style="text-align:left;padding:4px 6px">Date</th><th style="text-align:left;padding:4px 6px">Dérive</th><th></th></tr></thead><tbody>${rows.join('')}</tbody></table>`;
    blHost.querySelectorAll('[data-del-bl]').forEach((b) => {
      b.onclick = async () => { await api.deleteBaseline(b.dataset.delBl); renderDashboard(); };
    });
  }).catch(() => { blHost.innerHTML = '<span class="muted">—</span>'; });

  host.querySelector('#dash-new-baseline').onclick = async () => {
    const { promptDialog } = await import('../ui/modal.js');
    const name = await promptDialog('Nom de la baseline', `Semaine ${new Date().toISOString().slice(0, 10)}`);
    if (!name) return;
    await api.createBaseline(state.board.id, name);
    toast('Baseline enregistrée', 'ok');
    renderDashboard();
  };

  host.querySelector('#dash-ai-sum')?.addEventListener('click', async () => {
    const out = host.querySelector('#dash-ai-out');
    out.textContent = 'Génération du résumé par Apple Intelligence…';
    try {
      const r = await api.aiSummarize(state.board.id);
      out.innerHTML = `<pre>${escapeHtml(r.summary)}</pre>`;
    } catch (e) {
      out.textContent = `IA indisponible : ${e.message}`;
    }
  });
}
