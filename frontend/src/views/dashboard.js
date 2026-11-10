// Vue Dashboard : statistiques, graphiques SVG, baselines, résumé IA.

import { api } from '../api.js';
import { escapeHtml, fmtDateShort, parseDate, dueState } from '../format.js';
import { state, visibleLists } from '../state.js';
import { toast } from '../ui/modal.js';

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
      <div class="stat"><div class="v">${cards.length}</div><div class="k">cartes actives</div></div>
      <div class="stat ${overdue.length ? 'err' : ''}"><div class="v">${overdue.length}</div><div class="k">en retard</div></div>
      <div class="stat ${dueSoon.length ? 'warn' : ''}"><div class="v">${dueSoon.length}</div><div class="k">échéance ≤ 2 j</div></div>
      <div class="stat"><div class="v">${done.length}</div><div class="k">terminées</div></div>
      <div class="stat"><div class="v">${avgPct}%</div><div class="k">avancement moyen</div></div>
    </div>

    <div class="charts">
      <div class="panel">
        <h3>Répartition par liste</h3>
        ${perList.map((p) => `
          <div class="hbar">
            <span class="lbl">${escapeHtml(p.label)}${p.wip ? ` <span class="muted">WIP ${p.wip}</span>` : ''}</span>
            <div class="bar" style="width:${Math.round((p.count / maxList) * 100)}%; background:${p.wip && p.count > p.wip ? 'var(--warning)' : 'var(--c1)'}"></div>
            <b>${p.count}</b>
          </div>`).join('') || '<span class="muted">Aucune liste</span>'}
      </div>

      <div class="panel">
        <h3>Priorités</h3>
        ${perPriority.filter((p) => p.count).map((p) => `
          <div class="hbar">
            <span class="lbl" style="color:${prioColors[p.p]}">${p.p}</span>
            <div class="bar" style="width:${Math.round((p.count / maxPrio) * 100)}%;background:${prioColors[p.p]}"></div>
            <b>${p.count}</b>
          </div>`).join('') || '<span class="muted">Aucune priorité définie</span>'}
        ${overdue.length ? `
          <h3 style="margin-top:16px">En retard</h3>
          ${overdue.slice(0, 6).map((c) => `<div class="hbar"><span class="lbl">⚠ ${escapeHtml(c.title)}</span><span class="badge overdue">${fmtDateShort(c.due)}</span></div>`).join('')}` : ''}
      </div>

      <div class="panel">
        <h3>Jalons à venir</h3>
        ${milestones.slice(0, 8).map((m) => `
          <div class="hbar"><span class="lbl">◆ ${escapeHtml(m.title)}</span><span class="badge ${m.due ? '' : 'muted'}">${m.due ? fmtDateShort(m.due) : 'sans date'}</span></div>`).join('') || '<span class="muted">Aucun jalon</span>'}
      </div>

      <div class="panel">
        <h3>Baselines (Project)</h3>
        <div id="dash-baselines"><span class="muted">Chargement…</span></div>
        <div class="row" style="margin-top:10px">
          <button class="btn sm" id="dash-new-baseline">＋ Créer une baseline</button>
        </div>
      </div>
    </div>

    <div class="panel ai-panel">
      <div class="ai-head"><div class="ai-orb">✨</div><h3 style="margin:0">Résumé IA du board</h3>
        <div style="flex:1"></div>
        ${state.aiStatus?.available
          ? '<button class="btn sm primary" id="dash-ai-sum">Générer</button>'
          : '<span class="badge muted">IA indisponible sur cette machine</span>'}
      </div>
      <div id="dash-ai-out" class="muted" style="font-size:13px">${state.aiStatus?.available
        ? 'Apple Intelligence résume l\'avancement, les risques et les échéances critiques.'
        : 'Apple Intelligence requiert un Mac Apple Silicon avec l\'option activée (Réglages › Apple Intelligence &amp; Siri). Les données restent 100 % locales quand elle est disponible.'}</div>
    </div>`;

  // baselines
  const blHost = host.querySelector('#dash-baselines');
  api.baselines(state.board.id).then(async (bases) => {
    if (!bases.length) { blHost.innerHTML = '<span class="muted">Aucune baseline — créez un instantané pour suivre les dérives.</span>'; return; }
    const rows = await Promise.all(bases.map(async (b) => {
      let late = 0, total = 0;
      try {
        const diffs = await api.compareBaseline(b.id);
        total = diffs.filter((d) => d.due_baseline).length;
        late = diffs.filter((d) => (d.delta_due_days || 0) > 0).length;
      } catch { /* ignore */ }
      return `<tr><td>${escapeHtml(b.name)}</td><td class="muted">${fmtDateShort(b.created_at)}</td>
        <td>${late}/${total} en dérive</td>
        <td><button class="iconbtn" data-del-bl="${b.id}" style="width:22px;height:22px;font-size:11px">🗑</button></td></tr>`;
    }));
    blHost.innerHTML = `<table><thead><tr><th>Nom</th><th>Date</th><th>Dérive</th><th></th></tr></thead><tbody>${rows.join('')}</tbody></table>`;
    blHost.querySelectorAll('[data-del-bl]').forEach((b) => {
      b.onclick = async () => { await api.deleteBaseline(b.dataset.delBl); renderDashboard(); };
    });
  }).catch(() => { blHost.innerHTML = '<span class="muted">—</span>'; });

  host.querySelector('#dash-new-baseline').onclick = async () => {
    const { promptDialog } = await import('../ui/modal.js');
    const name = await promptDialog('Nom de la baseline', `Semaine ${new Date().toISOString().slice(0, 10)}`);
    if (!name) return;
    await api.createBaseline(state.board.id, name);
    toast('Baseline créée', 'ok');
    renderDashboard();
  };

  host.querySelector('#dash-ai-sum')?.addEventListener('click', async () => {
    const out = host.querySelector('#dash-ai-out');
    out.textContent = 'Génération du résumé…';
    try {
      const r = await api.aiSummarize(state.board.id);
      out.innerHTML = `<pre>${escapeHtml(r.summary)}</pre>`;
    } catch (e) {
      out.textContent = `IA indisponible : ${e.message}`;
    }
  });
}
