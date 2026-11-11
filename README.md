# Krello

**Clone local de Trello & Microsoft Project** — mono-utilisateur, orienté Kanban,
backend Rust, app native macOS & Windows, Apple Intelligence on-device.

![vues](https://img.shields.io/badge/vues-Kanban%C2%B7Calendrier%C2%B7Gantt%C2%B7Table%C2%B7Dashboard-6366f1)
![backend](https://img.shields.io/badge/backend-Rust%20%2B%20SQLite%20(FTS5%2C%20CPM)-22d3ee)
![ia](https://img.shields.io/badge/IA-Apple%20Intelligence%20on--device-f472b6)

## Fonctions

- **Kanban** : drag & drop fluide, listes, WIP limits, labels, couvertures,
  checklists avec progression, champs personnalisés, pièces jointes, commentaires,
  dates + priorités + filtres, recherche plein-texte, archivage, duplication,
  import/export (JSON/MD/CSV/ICS) et **import d'exports Trello**.
  **Suivi du temps** : ▶/⏸ sur une carte banque les minutes travaillées
  (incrément cumulé, session en cours affichée live) — migration 005.
  **Historique par carte** : section « Activité » dans la modale (flux filtré
  `card_id`, libellés FR, détail du payload).
- **Gantt / Timeline (MS Project)** : durées en jours ouvrés, charge, % accompli,
  jalons, dépendances **FS/SS/FF/SF + lag**, **chemin critique** (CPM complet :
  marges, ES/EF/LS/LF), calendriers avec jours fériés, baselines et dérives, WBS,
  zoom jour/semaine/mois, drag pour replanifier.
- **Calendrier** mensuel (drag pour replanifier), **Table** triable éditable,
  **Dashboard** (stats, graphiques, baselines, résumé IA).
- **Automatisations** locales (Butler-like) : 13 déclencheurs, 15 actions dont
  **cross-board** (copier/créer/déplacer vers un autre board), templates
  `{{card.title}}`…, journal d'exécution — voir [`docs/AUTOMATIONS.md`](docs/AUTOMATIONS.md).
  Sweep planifié chaque minute. **Rappels effectifs** : `reminder` (`15m`/`1h`/`1d`/`2d`)
  déclenche un événement WebSocket `ReminderDue` + toast, une seule fois par carte/échéance.
- **Undo/redo ⌘Z** (100 opérations), **palette ⌘K**, thèmes clair/sombre,
  WebSocket temps réel, tout en français.
- **Sauvegardes automatiques** : répertoire au choix dans les Paramètres
  (sélecteur natif Finder/Explorer dans l'app Krello, saisie manuelle en mode
  navigateur), intervalle (1 h → 1 semaine) et nombre de copies conservées
  configurables ; snapshot SQLite compact via `VACUUM INTO`
  (`krello-backup-<date>.db`), rotation automatique, sauvegarde manuelle en un clic.
- **Apple Intelligence** (macOS 15.1+, Apple Silicon) : carte/plan de projet en
  langage naturel, résumés, priorités suggérées, reformulation, checklists —
  100 % on-device. Documenté dans [`docs/APPLE_INTELLIGENCE.md`](docs/APPLE_INTELLIGENCE.md).

## Compilation & lancement

Prérequis : Rust 1.85+ (edition 2021 ok), Xcode CLT (macOS). SQLite est compilé
en statique (`rusqlite bundled`), **aucune autre dépendance système**.

```bash
# tout vérifier
cargo test --workspace

# binaire CLI (serveur + interface, frontend embarqué)
cargo build --release -p krello-server
./target/release/krello --open          # http://127.0.0.1:8787
./target/release/krello --help          # --db --port --token --frontend --open --no-ai

# app native macOS (.app + .dmg) — nécessite npm une seule fois
npm i -g @tauri-apps/cli
cargo build -p krello-app               # binaire dev (fenêtre native)
npx @tauri-apps/cli build               # bundle com.krello.app

# Windows (sur la machine ou CI)
cargo build --release -p krello-server  # target\release\krello.exe
npx @tauri-apps/cli build               # bundle .msi
```

Données : `~/Library/Application Support/com.krello.app/` (macOS),
`%APPDATA%\Krello\` (Windows). Sécurité : écoute **127.0.0.1 uniquement**,
`--token` pour ajouter un Bearer.

### Mode développement / test (sans build release)

```bash
# app en mode dev : compile en debug (~5 s) + ouvre le navigateur
cargo run -p krello-server -- --open

# base isolée pour ne pas toucher aux vraies données
cargo run -p krello-server -- --open --db /tmp/krello-test.db

# frontend servi depuis le disque → JS/CSS pris en compte par simple rechargement
cargo run -p krello-server -- --open --db /tmp/krello-test.db --frontend frontend

# fenêtre native (comportement identique à l'app finale, sans bundler)
cargo run -p krello-app

# itération continue (recompile à chaque modification Rust)
cargo install cargo-watch
cargo watch -x 'run -p krello-server -- --open --db /tmp/krello-test.db'
```

Le frontend n'a **aucune étape de build** : modifie `frontend/src/*.js` puis
recharge la page. Seules les modifications Rust exigent une recompilation.

**Apple Intelligence** : active automatiquement sur Mac éligible (Apple Silicon +
macOS 15.1+ + Apple Intelligence activée) ; **jamais sur Windows** (helper compilé
en stub vide → fonctionnalités masquées dans l'UI). Désactivation manuelle sur
Mac : `krello --no-ai` ou variable `KRELLO_NO_AI=1`.

## Structure

```
crates/krello-core      domaine, SQLite+FTS5, CPM, search, exim, undo, AI trait
crates/krello-ai-helper compile (build.rs → xcrun swiftc) et embarque le helper
crates/krello-server    axum REST+WS+statique (binaire `krello`)
src-tauri               app native Tauri v2 (serveur in-process, icônes)
frontend/               interface ES modules sans build (français, thèmes)
docs/PLAN.md            plan complet (features, architecture, décisions)
docs/API.md             référence API
docs/APPLE_INTELLIGENCE.md  analyse pertinence/faisabilité + implémentation
.github/workflows/build.yml  CI macOS + Windows
```

## Raccourcis

`⌘K` palette · `⌘Z` / `⇧⌘Z` undo/redo · `N` **nouvelle carte en langage naturel**
(IA — hors champ de saisie, board ouvert, Apple Intelligence disponible uniquement) ·
`Échap` ferme modales · Entrée valide quick-add · glisser une carte du Gantt ou du
calendrier replanifie.

## Tests

110 tests (smoke, cartes/déplacements/undo, CPM+calendriers+baselines, recherche,
import/export/automations, IA, intégration HTTP, rappels, récurrence, tracking,
duplication de listes, garde Host, sauvegardes) : `cargo test --workspace`.

## Journal des modifications

### v0.1.6 — historique par carte + suivi du temps

- **Historique par carte** : `GET /boards/{id}/activity?card_id=` (filtre SQL
  `card_id`) + section « Activité » dans la modale carte (libellés FR des kinds,
  horodatage, détail du payload — déplacement, fichier, récurrence…).
- **Suivi du temps** (migration 005 : `time_spent_minutes`,
  `tracking_started_at`) : `PATCH {time_spent_minutes}` **banque un incrément**
  (jamais un remplacement), `PATCH {tracking_started_at: ""}` arrête la session
  et banque l'écart depuis le début ; UI ▶/⏸ dans la modale (badge en cours,
  cumul, rafraîchissement 30 s, purge à la fermeture) ; undo/redo exact par
  opération.

### v0.1.5 — audit complet : sécurité, perfs, undo étendu, accessibilité + features

- **Garde `Host` anti DNS-rebinding** : toute requête dont l'en-tête `Host`
  n'est pas loopback (`127.0.0.1`, `localhost`, `[::1]`, avec ou sans port)
  est rejetée **403** — conforme au plan §5. Sans en-tête `Host` (clients CLI)
  → autorisé.
- **404 JSON pour `/api/v1/*` inconnu** : le fallback SPA ne s'applique plus
  aux routes API (`{"error":"not_found"}` au lieu de `index.html` en 200).
- **Téléchargement des pièces jointes en streaming** (`ServeFile`) au lieu de
  bufferiser jusqu'à 64 Mo en mémoire ; support des requêtes `Range` (206).
  `Content-Type`/`Content-Disposition` validés et assainis (CRLF retiré).
- **N+1 éliminés** : snapshot en masse des cartes (8 requêtes fixes pour
  `delete_board`/`delete_list`/`export_json`), items de checklists en un
  `IN (...)`, exports MD/CSV sans `prepare` en boucle, un seul recompute CPM
  par `POST /schedule`, hydratation frontend en 1 requête, palette ⌘K
  debouncée.
- **Undo étendu** : calendriers, baselines (+snapshots), attachments
  (purge fichier best-effort) ; **récurrence** : la complétion d'une carte
  récurrente crée l'occurrence suivante, undoable en une opération.
- **Rappels effectifs** : `reminder` (`15m`/`1h`/`1d`/`2d`) → event WS
  `ReminderDue` + toast, une seule fois par carte/échéance, réarmé au
  changement de `due`/`reminder` (migration 004).
- **Déplacement inter-boards** dans l'UI (sélecteur board+liste dans la
  modale ; `CardMoved` émis pour les deux boards).
- **Duplication de liste** : cartes + labels/checklists/champs/WIP, pièces
  jointes **fichiers dupliqués physiquement**, deps non copiées, undo unifié
  (`POST /lists/{id}/duplicate`).
- **Pièces jointes par glisser-déposer** depuis le bureau (multi-fichiers).
- **Calendrier Mois/Semaine/Jour** (persisté) + vue **« Aujourd'hui »**
  (retard/aujourd'hui/semaine/prioritaires).
- **Accessibilité** : `role="dialog"` + `aria-modal` + `aria-label` auto +
  focus trap + restauration du focus + toasts `role="status"` + alternative
  clavier au DnD (`<` / `>`).
- **Token comparé en temps constant** ; `Content-Disposition` assaini (CRLF).
- **FIX** : `GET /views` renvoie les réglages déjà parsés — `main.js` ne
  re-parse plus (les réglages de vue n'étaient jamais chargés après reload :
  zoom/filtres perdus).

### v0.1.3 — raccourci `N` : carte en langage naturel

- **Touche `N`** (hors champ de saisie, board ouvert) ouvre le dialog
  « ✨ Nouvelle carte en langage naturel » : description libre + liste de
  destination → Apple Intelligence extrait **titre, échéance (date résolue),
  priorité, labels et checklist**, puis crée la carte ; le résultat parsé est
  affiché dans le dialog et le Kanban se met à jour via l'événement WebSocket.
- Chaînable : le dialog reste ouvert (« Créer une autre ») pour saisir plusieurs
  tâches d'affilée.
- Gating cohérent : `N` est **inopérant sans IA** (Windows, Mac non éligible,
  `--no-ai`) — aucun dialog, aucune requête ; l'action est aussi absente de la
  palette ⌘K. Disponible sinon via ✨ → « Carte en langage naturel ».
- Limite connue : la résolution des dates relatives (« vendredi prochain »)
  est faite par le modèle on-device et peut varier d'un jour — la date reste
  éditable sur la carte après création.

### v0.1.2 — verrouillage Apple Intelligence + fix d'affichage temps réel

**Gating strict Apple Intelligence** (macOS uniquement, masquée partout ailleurs) :

- Le statut IA (`/api/v1/ai/status`) est chargé **une seule fois au boot**
  (`state.aiStatus`) et pilote l'affichage de **tous** les points d'entrée :
  - bouton **✨** de la barre supérieure (tiroir IA : carte en langage naturel,
    plan de projet, résumé, priorités) ;
  - menu **✨** dans la modale de carte (checklist IA, reformulation, échéance
    suggérée) ;
  - bouton **« Générer »** du panneau « Résumé IA » du Dashboard — remplacé par
    un badge « IA indisponible sur cette machine » + explication sinon.
- **Windows / Linux / Mac non éligible** : le helper Swift n'est pas compilé
  (stub vide dans `krello-ai-helper`), le backend répond
  `{"available": false, "reason": "unsupported_platform"}` et l'UI masque
  intégralement l'IA — aucune requête IA ne peut aboutir.
- **Désactivation manuelle sur Mac** : `krello --no-ai` ou `KRELLO_NO_AI=1`
  (raison renvoyée : `disabled`) — utile pour tester l'UI sans IA.
- Raisons machine-lisibles documentées dans
  [`docs/APPLE_INTELLIGENCE.md`](docs/APPLE_INTELLIGENCE.md) :
  `device_not_eligible`, `apple_intelligence_not_enabled`, `model_not_ready`,
  `os_too_old`, `helper_not_compiled`, `disabled`.

**Fix : les listes/cartes créées via l'UI s'affichent immédiatement** :

- *Cause* : lors d'un événement temps réel (WebSocket), le routeur sautait le
  rechargement des données si on restait sur le même board
  (`state.board.id !== boardId` → pas de `loadBoard`). L'état local
  (`state.lists`, `state.cards`…) restait donc périmé après une mutation
  serveur — la nouvelle liste n'apparaissait jamais sans changer de vue.
- *Correctif* :
  - `renderRoute` recharge **toujours** le board (6 requêtes SQLite locales,
    coût négligeable) ;
  - nouveau helper `refreshBoard()` (= `loadBoard` + hydratation
    labels/checklists) utilisé par toutes les vues après mutation ;
  - les vues Kanban / Calendrier / Timeline / Table re-rendent désormais sur
    des données fraîches.
- *Vérifié* sous Chromium headless : création de liste + quick-add de carte
  visibles immédiatement, zéro erreur console (commit `ed1b64e`).

**Frontend embarqué** : le binaire `krello` sert l'interface depuis des assets
compilés (`include_dir`) quand aucun dossier `frontend/` n'est présent — le
binaire est autonome (6,3 Mo), l'app Tauri fait de même via `frontendDist`.
