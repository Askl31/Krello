// Vue Paramètres Généraux : Apparence, Sauvegardes automatiques, Calendriers CPM, Apple Intelligence, macOS.
// (Les paramètres spécifiques aux tableaux sont désormais gérés dans la fenêtre dédiée accessible depuis chaque tableau).

import { api } from '../api.js';
import { escapeHtml } from '../format.js';
import { state, applyTheme, applyStyle, loadBoards, applyBoardBackground } from '../state.js';
import { toast, confirmDialog, promptDialog, openModal } from '../ui/modal.js';
import { navigate } from '../router.js';
import { icon } from '../icons.js';
import { BOARD_BACKGROUNDS, openBoardSettingsModal } from '../components/board_settings_modal.js';

export { BOARD_BACKGROUNDS };

export function renderSettings() {
  const host = document.querySelector('.view');
  if (!host) return;
  const isMac = state.system?.platform === 'macos';
  let tab = state.settingsTab || 'appearance';
  // Si un ancien onglet lié au tableau était actif, basculer sur 'appearance'
  if (tab === 'board' || tab === 'automations') tab = 'appearance';
  if (!isMac && tab === 'macos') tab = 'appearance';

  let tabContentHtml = '';

  if (tab === 'appearance') {
    tabContentHtml = `
      <div class="settings-card">
        <div class="settings-card-head">
          <div>
            <h3 class="settings-card-title">${icon('theme', { size: 18, className: 'muted' })} <span>Mode d'affichage</span></h3>
            <p class="settings-card-desc">Choisissez l'apparence générale de Krello ou synchronisez-la automatiquement avec macOS.</p>
          </div>
        </div>
        <div class="theme-picker-grid" id="st-theme-picker">
          <div class="theme-choice-card ${state.theme === 'dark' ? 'active' : ''}" data-theme="dark">
            <div class="theme-mock-preview theme-mock-dark">
              <div class="mock-bar"><div class="mock-dot"></div></div>
              <div class="mock-body"><div class="mock-col"></div><div class="mock-col"></div><div class="mock-col"></div></div>
            </div>
            <div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:4px">
              <b style="font-size:13.5px">${icon('moon', { size: 14 })} Sombre</b>
              ${state.theme === 'dark' ? '<span class="badge done" style="font-size:10.5px">● Actif</span>' : ''}
            </div>
            <span class="muted" style="font-size:12px;line-height:1.4">Contraste profond façon Obsidian, reposant pour les yeux.</span>
          </div>

          <div class="theme-choice-card ${state.theme === 'light' ? 'active' : ''}" data-theme="light">
            <div class="theme-mock-preview theme-mock-light">
              <div class="mock-bar"><div class="mock-dot"></div></div>
              <div class="mock-body"><div class="mock-col"></div><div class="mock-col"></div><div class="mock-col"></div></div>
            </div>
            <div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:4px">
              <b style="font-size:13.5px">${icon('sun', { size: 14 })} Clair</b>
              ${state.theme === 'light' ? '<span class="badge done" style="font-size:10.5px">● Actif</span>' : ''}
            </div>
            <span class="muted" style="font-size:12px;line-height:1.4">Style Studio épuré et lumineux, haute clarté visuelle.</span>
          </div>

          <div class="theme-choice-card ${state.theme === 'auto' ? 'active' : ''}" data-theme="auto">
            <div class="theme-mock-preview theme-mock-auto">
              <div class="mock-bar"><div class="mock-dot"></div></div>
              <div class="mock-body"><div class="mock-col dark"></div><div class="mock-col light"></div></div>
            </div>
            <div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:4px">
              <b style="font-size:13.5px">${icon('monitor', { size: 14 })} Système macOS</b>
              ${state.theme === 'auto' ? '<span class="badge done" style="font-size:10.5px">● Actif</span>' : ''}
            </div>
            <span class="muted" style="font-size:12px;line-height:1.4">S'adapte automatiquement à l'apparence configurée sur votre Mac.</span>
          </div>
        </div>
      </div>

      <div class="settings-card">
        <div class="settings-card-head">
          <div>
            <h3 class="settings-card-title">${icon('layout', { size: 18, className: 'muted' })} <span>Style d'interface (Design System)</span></h3>
            <p class="settings-card-desc">Basculez entre le design Standard et le design Translucide Liquid Glass façon macOS / visionOS.</p>
          </div>
        </div>
        <div class="style-picker-grid" id="st-style-picker">
          <div class="style-choice-card ${state.style === 'standard' ? 'active' : ''}" data-style="standard">
            <div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:6px">
              <b style="font-size:14px">${icon('kanban', { size: 14 })} Design Standard</b>
              <span class="style-badge-tag">${state.style === 'standard' ? '● Actif' : 'Standard'}</span>
            </div>
            <p class="muted" style="font-size:12.5px;line-height:1.5;margin:0 0 10px">
              Fonds opaques réguliers, contrastes marqués inspirés de Linear. Optimisé pour la performance maximale et une concentration sans distraction.
            </p>
            <div class="row" style="font-size:11.5px;color:var(--fg-secondary);gap:8px">
              <span>✓ Contrastes nets</span>
              <span>✓ Surfaces opaques</span>
              <span>✓ Rendu ultra-rapide</span>
            </div>
          </div>

          <div class="style-choice-card ${state.style === 'translucent' ? 'active' : ''}" data-style="translucent">
            <div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:6px">
              <b style="font-size:14px">${icon('ai', { size: 14 })} Translucide Liquid Glass</b>
              <span class="style-badge-tag">${state.style === 'translucent' ? '● Actif' : 'Liquid Glass'}</span>
            </div>
            <p class="muted" style="font-size:12.5px;line-height:1.5;margin:0 0 10px">
              Matériaux dépolis en temps réel, flou d'arrière-plan adaptatif et reflets de lumière subtils inspirés de macOS Sonoma et visionOS.
            </p>
            <div class="row" style="font-size:11.5px;color:var(--fg-secondary);gap:8px">
              <span>✓ Verre dépoli dynamique</span>
              <span>✓ Reflets subtils</span>
              <span>✓ Intégration ambiances</span>
            </div>
          </div>
        </div>

        <div class="settings-persisted-pill">
          ${icon('check', { size: 14 })} <span>Vos préférences d'interface sont enregistrées en base SQLite locale et conservées à chaque démarrage.</span>
        </div>
      </div>`;
  } else if (tab === 'backups') {
    tabContentHtml = `
      <div class="settings-card">
        <div class="settings-card-head">
          <div>
            <h3 class="settings-card-title">${icon('import', { size: 18, className: 'muted' })} <span>Sauvegardes automatiques (SQLite VACUUM INTO)</span></h3>
            <p class="settings-card-desc">
              Copies compactes et cohérentes de toute la base (tableaux, cartes, listes, labels, automations) sans aucun risque de corruption.
              Rotation automatique au-delà du quota configuré.
            </p>
          </div>
        </div>
        <div class="field">
          <label>Répertoire de destination <span id="st-backup-dir-hint" class="muted" style="font-size:11.5px">(le sélecteur natif est disponible dans l'app Krello)</span></label>
          <div class="row" style="gap:8px">
            <input class="input" id="st-backup-dir" placeholder="/Users/moi/Sauvegardes/krello" style="flex:1" />
            <button class="btn ghost sm" id="st-backup-browse" style="flex:none;display:none">📁 Parcourir…</button>
          </div>
        </div>
        <div class="row" style="gap:12px;margin-top:10px;flex-wrap:wrap">
          <div class="field" style="flex:1;min-width:180px">
            <label>Intervalle automatique</label>
            <select class="input" id="st-backup-interval">
              <option value="1">Toutes les heures</option>
              <option value="6">Toutes les 6 heures</option>
              <option value="12">Toutes les 12 heures</option>
              <option value="24">Une fois par jour</option>
              <option value="168">Une fois par semaine</option>
            </select>
          </div>
          <div class="field" style="flex:1;min-width:180px">
            <label>Copies conservées</label>
            <input type="number" min="1" max="100" class="input" id="st-backup-keep" />
          </div>
        </div>
        <div class="row" style="gap:8px;margin-top:14px">
          <button class="btn primary sm" id="st-backup-save">${icon('check', { size: 13 })} <span>Enregistrer les réglages</span></button>
          <button class="btn ghost sm" id="st-backup-run">Sauvegarder maintenant</button>
        </div>
        <div id="st-backup-status" style="margin-top:12px"><span class="muted">…</span></div>
      </div>

      <div class="settings-card settings-danger-card">
        <div class="settings-card-head">
          <div>
            <h3 class="settings-card-title">${icon('alert', { size: 18 })} <span>Zone de danger globale</span></h3>
            <p class="settings-card-desc">Opération de suppression intégrale et définitive des données locales de l'application.</p>
          </div>
        </div>
        <div class="row" style="flex-wrap:wrap;gap:10px">
          <button class="btn danger sm" id="st-delete-all">${icon('trash', { size: 13 })} <span>Tout supprimer (tous les tableaux et cartes)</span></button>
        </div>
      </div>`;
  } else if (tab === 'calendars') {
    tabContentHtml = `
      <div class="settings-card">
        <div class="settings-card-head">
          <div>
            <h3 class="settings-card-title">${icon('calendar', { size: 18, className: 'muted' })} <span>Calendriers de travail partagés (moteur CPM)</span></h3>
            <p class="settings-card-desc">Définit les heures travaillées par jour et les jours fériés disponibles pour l'ensemble de vos projets.</p>
          </div>
          <button class="btn sm primary" id="st-add-calendar">${icon('plus', { size: 13 })} <span>Nouveau calendrier</span></button>
        </div>
        <div id="st-calendars" style="display:flex;flex-direction:column;gap:6px"></div>
      </div>`;
  } else if (tab === 'ai') {
    tabContentHtml = `
      <div class="settings-card">
        <div class="settings-card-head">
          <div>
            <h3 class="settings-card-title">${icon('ai', { size: 18, className: 'muted' })} <span>Moteur Apple Intelligence</span></h3>
            <p class="settings-card-desc">Assistant intelligent intégré exécuté à 100 % sur puce Apple Silicon locale (CoreML / Swift). Aucune donnée ne quitte votre machine.</p>
          </div>
        </div>
        <div id="st-ai" style="padding:4px 0 8px"><span class="muted">Vérification de la disponibilité du moteur…</span></div>
      </div>

      <div class="settings-card">
        <div class="settings-card-head">
          <div>
            <h3 class="settings-card-title">${icon('ai', { size: 18, className: 'muted' })} <span>Fonctionnalités IA intégrées</span></h3>
            <p class="settings-card-desc">Outils disponibles dans la barre supérieure, les fiches de cartes et le volet Copilot :</p>
          </div>
        </div>
        <div style="display:grid;grid-template-columns:repeat(auto-fit,minmax(260px,1fr));gap:12px;margin-top:10px">
          <div style="padding:12px;background:var(--surface-2);border-radius:var(--radius-md);border:1px solid var(--border)">
            <b style="font-size:13px;display:flex;align-items:center;gap:6px">${icon('kanban', { size: 14 })} Génération de projet</b>
            <p class="muted" style="font-size:12px;margin:6px 0 0;line-height:1.4">Transforme un objectif ou cahier des charges en listes, tâches ordonnées et dépendances CPM.</p>
          </div>
          <div style="padding:12px;background:var(--surface-2);border-radius:var(--radius-md);border:1px solid var(--border)">
            <b style="font-size:13px;display:flex;align-items:center;gap:6px">${icon('edit', { size: 14 })} Saisie en langage naturel</b>
            <p class="muted" style="font-size:12px;margin:6px 0 0;line-height:1.4">Extrait automatiquement le titre, l'échéance et la priorité à partir d'une phrase rapide.</p>
          </div>
          <div style="padding:12px;background:var(--surface-2);border-radius:var(--radius-md);border:1px solid var(--border)">
            <b style="font-size:13px;display:flex;align-items:center;gap:6px">${icon('timeline', { size: 14 })} Priorisation du backlog</b>
            <p class="muted" style="font-size:12px;margin:6px 0 0;line-height:1.4">Évalue et ordonne automatiquement les cartes selon leur urgence et le chemin critique.</p>
          </div>
          <div style="padding:12px;background:var(--surface-2);border-radius:var(--radius-md);border:1px solid var(--border)">
            <b style="font-size:13px;display:flex;align-items:center;gap:6px">${icon('check', { size: 14 })} Checklists automatiques</b>
            <p class="muted" style="font-size:12px;margin:6px 0 0;line-height:1.4">Décompose instantanément n'importe quelle carte complexe en sous-étapes logiques.</p>
          </div>
        </div>
      </div>`;
  } else if (tab === 'macos') {
    tabContentHtml = `
      <div class="settings-card">
        <div class="settings-card-head">
          <div>
            <h3 class="settings-card-title">${icon('ai', { size: 18, className: 'muted' })} <span>Siri &amp; Spotlight — ajout rapide de tâches</span></h3>
            <p class="settings-card-desc">
              Générez des raccourcis macOS (un par liste) pour créer une tâche dans une liste déterminée
              depuis <b>Spotlight</b> (⌘Espace → « Krello ») ou <b>Siri</b> (« Ajouter à Siri » dans l'app
              Raccourcis, puis dictez la tâche), sans ouvrir Krello.
            </p>
          </div>
        </div>
        <div class="field">
          <label>Dossier de génération des raccourcis</label>
          <div class="row" style="gap:8px">
            <input class="input" id="st-siri-dir" placeholder="/Users/moi/Raccourcis-Krello" style="flex:1" />
            <button class="btn ghost sm" id="st-siri-browse" style="flex:none;display:none">📁 Parcourir…</button>
            <button class="btn primary sm" id="st-siri-gen" style="flex:none">${icon('check', { size: 13 })} <span>Générer les raccourcis</span></button>
          </div>
          <p class="muted" style="font-size:12px;margin-top:6px">
            Écrit un fichier <code>.shortcut</code> par liste + <code>LISEZMOI.txt</code> dans ce dossier.
            Importez chaque fichier par double-clic (« Ajouter le raccourci »), puis dans l'app Raccourcis :
            « Ajouter à Siri » avec la phrase de votre choix. Les raccourcis apparaissent aussi dans Spotlight.
          </p>
        </div>
        <div id="st-siri-status" style="margin-top:12px"><span class="muted">…</span></div>
      </div>

      <div class="settings-card">
        <div class="settings-card-head">
          <div>
            <h3 class="settings-card-title">${icon('settings', { size: 18, className: 'muted' })} <span>Ligne de commande (repli / scripts)</span></h3>
            <p class="settings-card-desc">Binaire courant : <code id="st-siri-exe" class="muted"></code></p>
          </div>
        </div>
        <div id="st-siri-lists" style="display:flex;flex-direction:column;gap:6px"><span class="muted">Chargement…</span></div>
      </div>`;
  }

  host.innerHTML = `
    <div class="settings-container">
      <div class="settings-header">
        <h2>Paramètres généraux</h2>
        <p>Configurez l'application Krello : apparence globale, sauvegardes locales, calendriers de travail partagés et intégrations système.</p>
      </div>

      <div class="settings-board-info-banner">
        <div style="display:flex;align-items:center;gap:12px;min-width:0;flex:1">
          <div class="dialog-badge" style="width:36px;height:36px;flex:none">${icon('kanban', { size: 16 })}</div>
          <div style="font-size:12.5px;line-height:1.45">
            <b style="color:var(--fg);font-size:13px">Paramètres spécifiques à un tableau</b>
            <span class="muted" style="display:block">L'ambiance, la planification CPM, les labels, les champs personnalisés, les règles Butler et les exports se règlent désormais directement depuis la fenêtre de paramètres de chaque tableau.</span>
          </div>
        </div>
        ${state.boards?.length ? `
          <button class="btn sm ghost" id="st-open-board-settings-btn" style="flex:none">
            ${icon('settings', { size: 13 })} <span>Configurer un tableau…</span>
          </button>
        ` : ''}
      </div>

      <div class="settings-tabs-bar" id="st-tabs">
        <button class="settings-tab-btn ${tab === 'appearance' ? 'active' : ''}" data-tab="appearance">
          ${icon('theme', { size: 15 })} <span>Apparence</span>
        </button>
        <button class="settings-tab-btn ${tab === 'backups' ? 'active' : ''}" data-tab="backups">
          ${icon('import', { size: 15 })} <span>Sauvegardes</span>
        </button>
        <button class="settings-tab-btn ${tab === 'calendars' ? 'active' : ''}" data-tab="calendars">
          ${icon('calendar', { size: 15 })} <span>Calendriers</span>
        </button>
        <button class="settings-tab-btn ${tab === 'ai' ? 'active' : ''}" data-tab="ai">
          ${icon('ai', { size: 15 })} <span>Apple Intelligence</span>
        </button>
        ${isMac ? `<button class="settings-tab-btn ${tab === 'macos' ? 'active' : ''}" data-tab="macos">
          ${icon('ai', { size: 15 })} <span>macOS</span>
        </button>` : ''}
      </div>

      <div class="settings-content">
        ${tabContentHtml}
      </div>
    </div>`;

  // --- Gestion du bouton "Configurer un tableau…"
  host.querySelector('#st-open-board-settings-btn')?.addEventListener('click', () => {
    if (state.board) {
      openBoardSettingsModal(state.board.id);
    } else if (state.boards.length === 1) {
      openBoardSettingsModal(state.boards[0].id);
    } else if (state.boards.length > 1) {
      const { el: pickEl, close: pickClose } = openModal(`
        <div class="dialog-header">
          <div class="dialog-badge">${icon('kanban', { size: 18 })}</div>
          <div class="dialog-titles">
            <h3 class="dialog-title">Choisir un tableau</h3>
            <p class="dialog-desc">Sélectionnez le tableau dont vous souhaitez ouvrir les paramètres dédiés :</p>
          </div>
          <button class="iconbtn dialog-close" data-close title="Fermer (Échap)">${icon('close', { size: 14 })}</button>
        </div>
        <div class="dialog-body" style="gap:8px">
          ${state.boards.map((b) => `
            <button class="btn ghost sm" data-pick-bid="${b.id}" style="justify-content:flex-start;padding:10px 14px;font-size:13px;width:100%;text-align:left">
              ${icon('kanban', { size: 15 })} <b>${escapeHtml(b.title)}</b>
            </button>
          `).join('')}
        </div>
      `, { size: 'sm' });

      pickEl.querySelectorAll('[data-pick-bid]').forEach((btn) => {
        btn.onclick = () => {
          pickClose();
          openBoardSettingsModal(btn.dataset.pickBid);
        };
      });
    }
  });

  // --- Gestion du changement d'onglets
  host.querySelector('#st-tabs')?.addEventListener('click', (e) => {
    const btn = e.target.closest('[data-tab]');
    if (!btn) return;
    state.settingsTab = btn.dataset.tab;
    renderSettings();
  });

  // =========================================================================
  // Onglet Apparence : Thème & Style
  // =========================================================================
  host.querySelector('#st-theme-picker')?.addEventListener('click', (e) => {
    const card = e.target.closest('[data-theme]');
    if (!card) return;
    state.theme = card.dataset.theme;
    applyTheme(true);
    const tb = document.getElementById('topbar');
    if (tb) import('../components/topbar.js').then((m) => m.renderTopbar(tb));
    const sb = document.getElementById('sidebar');
    if (sb) import('../components/sidebar.js').then((m) => m.renderSidebar(sb));
    renderSettings();
    toast('Thème d\'apparence enregistré et appliqué', 'ok');
  });

  host.querySelector('#st-style-picker')?.addEventListener('click', (e) => {
    const card = e.target.closest('[data-style]');
    if (!card) return;
    state.style = card.dataset.style;
    applyStyle(true);
    const tb = document.getElementById('topbar');
    if (tb) import('../components/topbar.js').then((m) => m.renderTopbar(tb));
    const sb = document.getElementById('sidebar');
    if (sb) import('../components/sidebar.js').then((m) => m.renderSidebar(sb));
    renderSettings();
    toast(`Style ${state.style === 'translucent' ? 'Translucide Liquid Glass' : 'Standard'} appliqué et enregistré`, 'ok');
  });

  // =========================================================================
  // Onglet IA
  // =========================================================================
  const stAi = host.querySelector('#st-ai');
  if (stAi) {
    api.aiStatus().then((s) => {
      stAi.innerHTML = s.available
        ? `<div class="row" style="gap:8px">
            <span class="badge done" style="font-size:12px">● Moteur opérationnel (${escapeHtml(s.backend)})</span>
          </div>
          <p class="muted" style="margin:8px 0 0;font-size:12.5px;line-height:1.5">
            Apple Intelligence s'exécute directement sur le Neural Engine de votre Mac. Confidentialité intégrale garantie sans transfert de données réseau.
          </p>`
        : `<div class="row" style="gap:8px">
            <span class="badge overdue" style="font-size:12px">● Moteur non disponible</span>
          </div>
          <p class="muted" style="margin:8px 0 0;font-size:12.5px;line-height:1.5">
            ${escapeHtml(s.reason || '')} — Nécessite macOS 15.1+ sur Mac Apple Silicon avec Apple Intelligence activée dans Réglages Système.
          </p>`;
    }).catch(() => {
      stAi.innerHTML = '<span class="muted" style="font-size:12px">Statut indisponible</span>';
    });
  }

  // =========================================================================
  // Onglet Sauvegardes & Données
  // =========================================================================
  const stBackupStatus = host.querySelector('#st-backup-status');
  const stBackupDir = host.querySelector('#st-backup-dir');
  const stBackupInterval = host.querySelector('#st-backup-interval');
  const stBackupKeep = host.querySelector('#st-backup-keep');
  const stBackupBrowse = host.querySelector('#st-backup-browse');
  const stBackupDirHint = host.querySelector('#st-backup-dir-hint');
  const inNativeApp = typeof window !== 'undefined' && !!window.__TAURI_INTERNALS__;

  if (stBackupBrowse && inNativeApp) {
    stBackupBrowse.style.display = '';
    if (stBackupDirHint) stBackupDirHint.textContent = '(cliquez sur Parcourir… pour choisir un dossier natif)';
    stBackupBrowse.addEventListener('click', async () => {
      stBackupBrowse.disabled = true;
      try {
        const r = await api.pickBackupDir();
        if (r.dir) {
          stBackupDir.value = r.dir;
          toast('Répertoire sélectionné — pensez à Enregistrer les réglages', 'ok');
        }
      } catch (e) { toast(e.message, 'err'); }
      finally { stBackupBrowse.disabled = false; }
    });
  }

  const backupStatusHtml = (s) => s.last_backup_at
    ? `<span class="badge done">● Dernière sauvegarde : ${escapeHtml(s.last_backup_at.slice(0, 16).replace('T', ' '))}</span> <span class="muted" style="font-size:12px;margin-left:8px">${escapeHtml(s.last_backup_file || '')}</span>`
    : '<span class="muted" style="font-size:12px">Aucune sauvegarde enregistrée pour l\'instant.</span>';

  if (stBackupDir) {
    api.backupSettings().then((s) => {
      if (stBackupDir) stBackupDir.value = s.backup_dir || '';
      if (stBackupInterval) stBackupInterval.value = String(s.interval_hours);
      if (stBackupKeep) stBackupKeep.value = String(s.keep);
      if (stBackupStatus) stBackupStatus.innerHTML = backupStatusHtml(s);
    }).catch((e) => {
      if (stBackupStatus) stBackupStatus.innerHTML = `<span class="muted">${escapeHtml(e.message)}</span>`;
    });
  }

  host.querySelector('#st-backup-save')?.addEventListener('click', async () => {
    try {
      const s = await api.saveBackupSettings({
        backup_dir: stBackupDir?.value ?? '',
        interval_hours: Number(stBackupInterval?.value || 24),
        keep: Number(stBackupKeep?.value || 10),
      });
      toast('Réglages de sauvegarde enregistrés', 'ok');
      if (stBackupStatus) stBackupStatus.innerHTML = backupStatusHtml(s);
    } catch (e) { toast(e.message, 'err'); }
  });

  host.querySelector('#st-backup-run')?.addEventListener('click', async () => {
    try {
      await api.saveBackupSettings({
        backup_dir: stBackupDir?.value ?? '',
        interval_hours: Number(stBackupInterval?.value || 24),
        keep: Number(stBackupKeep?.value || 10),
      });
      const r = await api.runBackup();
      toast(`Sauvegarde créée (${Math.max(1, Math.round(r.size / 1024))} Ko)`, 'ok');
      if (stBackupStatus) {
        stBackupStatus.innerHTML = `<span class="badge done">● Dernière : maintenant</span> <span class="muted" style="font-size:12px;margin-left:8px">${escapeHtml(r.file)}</span>`;
      }
    } catch (e) { toast(e.message, 'err'); }
  });

  // Zone de danger globale : tout supprimer
  host.querySelector('#st-delete-all')?.addEventListener('click', async () => {
    const n = state.boards.length;
    if (n === 0) { toast('Aucun tableau à supprimer', 'err'); return; }
    if (!(await confirmDialog(`⚠ TOUT SUPPRIMER : supprimer définitivement les ${n} tableau${n > 1 ? 's' : ''} (listes, cartes, labels, champs, règles, commentaires, pièces jointes) ? Annulable uniquement avec ⌘Z pendant cette session.`))) return;
    try {
      const r = await api.deleteAllBoards();
      await loadBoards();
      state.board = null;
      state.lists = []; state.cards = []; state.labels = []; state.fields = []; state.deps = []; state.schedule = null;
      applyBoardBackground(null);
      toast(`${r.deleted} tableau(x) supprimé(s)`, 'ok');
      navigate('#/');
      renderSettings();
    } catch (e) { toast(e.message, 'err'); }
  });

  // =========================================================================
  // Onglet Calendriers partagés
  // =========================================================================
  const calHost = host.querySelector('#st-calendars');
  if (calHost) {
    api.calendars().then((cals) => {
      calHost.innerHTML = cals.map((c) => `
        <div class="row" style="padding:6px 10px;border-radius:var(--radius-xs);background:var(--surface-2);gap:10px">
          <b style="flex:1;font-size:13px">${escapeHtml(c.name)}</b>
          <span class="muted" style="font-size:12px">${c.hours_per_day} h / jour</span>
          <button class="iconbtn" data-del-cal="${c.id}" ${c.id === 'default' ? 'disabled style="opacity:.3"' : ''} title="Supprimer ce calendrier" style="width:22px;height:22px">${icon('trash', { size: 12 })}</button>
        </div>`).join('');
      calHost.querySelectorAll('[data-del-cal]').forEach((b) => {
        b.onclick = async () => {
          try { await api.deleteCalendar(b.dataset.delCal); renderSettings(); }
          catch (e) { toast(e.message, 'err'); }
        };
      });
    }).catch(() => {});

    host.querySelector('#st-add-calendar')?.addEventListener('click', async () => {
      const name = await promptDialog('Nom du calendrier');
      if (!name) return;
      const holidays = await promptDialog('Jours fériés (JSON, ex. ["2027-01-01"])', '[]');
      try {
        await api.createCalendar({ name, holidays });
        renderSettings();
      } catch (e) { toast(e.message, 'err'); }
    });
  }

  // =========================================================================
  // Onglet macOS : raccourcis Siri/Spotlight + commandes CLI par liste
  // =========================================================================
  if (isMac) {
    const stSiriDir = host.querySelector('#st-siri-dir');
    const stSiriBrowse = host.querySelector('#st-siri-browse');
    const stSiriGen = host.querySelector('#st-siri-gen');
    const stSiriStatus = host.querySelector('#st-siri-status');
    const stSiriLists = host.querySelector('#st-siri-lists');
    const stSiriExe = host.querySelector('#st-siri-exe');
    if (stSiriDir) {
      if (stSiriBrowse && inNativeApp) {
        stSiriBrowse.style.display = '';
        stSiriBrowse.addEventListener('click', async () => {
          try {
            const r = await api.pickSiriPackDir();
            if (r.dir) stSiriDir.value = r.dir;
          } catch (e) { toast(e.message, 'err'); }
        });
      }
      if (stSiriExe) stSiriExe.textContent = state.system?.exe || '';
      stSiriGen?.addEventListener('click', async () => {
        const dir = (stSiriDir.value || '').trim();
        if (!dir) { toast('Indiquez d’abord le dossier de génération', 'err'); return; }
        try {
          stSiriStatus.innerHTML = '<span class="muted">Génération…</span>';
          const r = await api.siriPack(dir);
          stSiriStatus.innerHTML = `<span class="badge done">● ${r.files.length} fichier(s) écrit(s) dans ${escapeHtml(r.dir)}</span>
            <ul style="margin:6px 0 0 18px">${r.files.map((f) => `<li class="muted" style="font-size:12px">${escapeHtml(f)}</li>`).join('')}</ul>`;
          toast(`${r.files.length} fichiers générés — importez-les par double-clic`, 'ok');
        } catch (e) {
          stSiriStatus.innerHTML = `<span class="muted">${escapeHtml(e.message)}</span>`;
          toast(e.message, 'err');
        }
      });
    }
    if (stSiriLists) {
      (async () => {
        try {
          const exe = state.system?.exe || 'krello';
          const boards = await api.boards();
          const rows = [];
          for (const b of boards) {
            const lists = await api.lists(b.id);
            for (const l of lists) {
              const cmd = `"${exe}" task add --title "…" --list ${JSON.stringify(l.title)}`;
              rows.push(`<div class="row" style="gap:8px;align-items:center;padding:4px 6px">
                <span class="muted" style="font-size:11.5px;flex:none">${escapeHtml(b.title)}</span>
                <b style="font-size:13px;flex:none">${escapeHtml(l.title)}</b>
                <code class="muted" style="flex:1;font-size:11.5px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap" title="${escapeHtml(cmd)}">${escapeHtml(cmd)}</code>
                <button class="btn ghost sm" data-copy="${escapeHtml(cmd)}" style="flex:none" title="Copier la commande">${icon('copy', { size: 13 })}</button>
              </div>`);
            }
          }
          stSiriLists.innerHTML = rows.length ? rows.join('') : '<span class="muted">Aucune liste active — créez-en une dans un tableau.</span>';
          stSiriLists.querySelectorAll('[data-copy]').forEach((btn) => {
            btn.addEventListener('click', async () => {
              try {
                await navigator.clipboard.writeText(btn.dataset.copy);
                toast('Commande copiée dans le presse-papiers', 'ok');
              } catch { toast('Copie impossible (presse-papiers indisponible)', 'err'); }
            });
          });
        } catch (e) {
          stSiriLists.innerHTML = `<span class="muted">${escapeHtml(e.message)}</span>`;
        }
      })();
    }
  }
}
