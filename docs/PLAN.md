# Plan complet — Krello, clone local de Trello & Microsoft Project

> Application locale mono-utilisateur, **orientée Kanban**, couvrant les vues de
> Trello (hors collaboration) et les fonctions d'ordonnancement de MS Project.
> Backend **Rust** (workspace Cargo), frontend web embarqué, app native **Tauri v2**
> macOS/Windows, **Apple Intelligence** on-device sur macOS.

## 1. Périmètre fonctionnel

### 1.1 Trello (version locale solo)

| Domaine | Fonctions |
|---|---|
| Boards | titre, description, fond (couleur), archivage, **suppression définitive** (sidebar + paramètres, undoable) et **tout supprimer** (zone de danger, `DELETE /boards`), duplication profonde (labels/listes/cartes/checklists/champs/dépendances/automations), activité récente, export JSON/MD/CSV/ICS, import Krello + **import export Trello**, **favoris épinglés** (`pinned`, tri sidebar), **modèles de board** (`is_board_template`, galerie « Nouveau board », instanciation = copie profonde, exclus de la liste des tableaux) |
| Listes | titre, position flottante, **limite WIP** (alerte UI), archivage |
| Cartes | titre, description Markdown, position, couverture (couleur/pièce jointe), **labels**, dates début + échéance (avec heure, « terminée », complétée à), priorité (5 niveaux), **checklists** (progression %), **champs personnalisés** (texte/nombre/date/case/select), **pièces jointes** fichiers locaux (64 Mo max, vignette cover), commentaires, duplication, archivage, déplacement intra/inter-listes **et inter-boards**, filtres (texte/label/priorité/échéance/retard), recherche plein-texte **FTS5** (+ **corps des commentaires** via `comments_fts` ; recherche globale incluant archivées), **modèles de cartes** (`is_template`, badge, galerie par liste, instanciation copie profonde `keepFromSource` — dates/statut/commentaires jamais copiés, exclus de recherche/compteurs/vues de travail), **sous-cartes hiérarchiques** (`parent_id`, même liste que le parent, WBS `1.1`, Kanban indenté, duplication récursive, détachement si le parent est archivé/supprimé, cycle → 400), **rappels multiples** (relatifs `15m→1w` avant échéance ou date fixe absolue, dédupliqués par `<idx>|<due>`, réarmés quand due/rappels changent) |
| Vues | **Kanban** (DnD pointer events, ghost, insertion par position, sous-cartes indentées sous leur parent), **Calendrier** (mois, drag pour replanifier l'échéance), **Timeline/Gantt** (barres CPM, jalons ◆, ligne aujourd'hui, zoom jour/semaine/mois, drag pour décaler, **flèches de dépendances**, chemin critique en rouge, marge affichée), **Table** (tri sur toutes les colonnes — WBS en tri naturel `1.2 < 1.10` — WBS, édition inline durée/charge/%), **Dashboard** (stats, répartitions, retards, jalons à venir, baselines + dérive, résumé IA), **Vue d'ensemble** (accueil : stats cross-board — retards, à venir 7 j, terminées 7 j —, rollup par tableau, activité récente) |
| Automatisations | moteur « bot » local façon Butler (voir `docs/AUTOMATIONS.md`) : 13 déclencheurs (événementiels + `card_due_soon`/`card_due` + `button` + `scheduled_daily/weekly/monthly`, sweep chaque minute) ; conditions JSON riches ; 15 actions **dont cross-board** `copy`/`create_card`/`move` vers n'importe quel board, labels recréés par nom, templates `{{card.title}}`… ; cascade inter-board bornée (profondeur 4) ; journal d'exécution `automation_runs` + endpoints `/runs` + UI constructeur dans les paramètres |
| UX | palette **⌘K** (boards, cartes, actions), **undo/redo ⌘Z/⇧⌘Z** (stack 100 opérations, redo), thèmes clair/sombre, toasts, WebSocket temps réel (maj incrémentale), raccourcis clavier |

### 1.2 Microsoft Project

| Domaine | Fonctions |
|---|---|
| Tâches | **durée (jours ouvrés)**, **charge (h)**, % accompli, **type** `fixed_duration/fixed_work/fixed_units`, jalon (durée 0, losange), **code WBS** auto (ordre listes/positions), coût |
| Dépendances | **FS / SS / FF / SF** + **lag** en jours (négatif = avance), unicité, détection de **cycles** (BFS → erreur 409 avec le chemin), édition/suppression |
| Calendrier | week-ends configurables (0=dim), jours fériés personnalisés, heures/jour ; partagé par board |
| Ordonnancement | **CPM complet** : passe avant (ES/EF), passe arrière (LS/LF), **marge totale**, **chemin critique** (marge 0), recalcul transactionnel à chaque changement pertinent, cache `schedule_cache`, respect des contraintes `start_not_before` et des débuts explicites |
| Baselines | instantanés nommés (start/due/durée/charge), comparaison avec **delta jours** sur l'échéance |
| Gantt | cf. vue Timeline ci-dessus |

### 1.3 Apple Intelligence (macOS) — voir `docs/APPLE_INTELLIGENCE.md`

Cas d'usage intégrés : carte en langage naturel, plan de projet NL → tâches + dépendances + dates,
résumé de board, suggestions de priorités (avec raisons), reformulation, génération de checklist.
Tous **on-device** via le framework Foundation Models, appelé par un **helper Swift embarqué**
(JSON-lines sur stdin/stdout), fallback gracieux si indisponible.

### 1.4 Siri & Spotlight (macOS) — voir `docs/MACOS_SIRI_SPOTLIGHT.md`

Ajout rapide d'une tâche dans une liste déterminée depuis **Spotlight** (⌘Espace)
ou **Siri** (phrase personnalisée) : Krello génère un pack de raccourcis
`.shortcut` (un par liste) qui déposent le titre dicté/saisi dans une inbox puis
ouvrent le deep link `krello://task/new?list=…` (app Tauri, plugin deep-link
macOS uniquement) — repli : CLI `krello task add --title … --list …` écrivant
directement dans la SQLite. Windows inchangé (plugin/config/capability macOS only).

## 2. Architecture

```
┌──────────────────────────── app native (Tauri v2) ───────────────────────────┐
│  WebView ──HTTP/WS──▶ serveur axum in-process (127.0.0.1, port éphémère)     │
└──────────────────────────────────────────────────────────────────────────────┘
┌──────────────────────────── binaire CLI `krello` ────────────────────────────┐
│  navigateur ──HTTP/WS──▶ serveur axum (127.0.0.1:8787, --frontend/--open)    │
└──────────────────────────────────────────────────────────────────────────────┘
                                   │
                            krello-core (Arc<Service>)
                     Mutex<rusqlite> · WAL · FTS5 · transactions
                                   │
                    krello-ai-helper (binaire Swift embarqué)
```

- **`krello-core`** : modèles (serde ↔ SQLite TEXT/ISO-8601), services domaine
  (boards/lists/cards/labels/checklists/fields/attachments/comments/deps/baselines/
  automations/calendars), scheduler CPM, recherche, import/export, moteur
  d'automatisations, undo/redo déclaratif (snapshots inverse), bus d'événements
  `tokio::sync::broadcast`, abstraction `AiProvider`.
- **`krello-ai-helper`** : compile `swift/main.swift` via `xcrun swiftc` (build.rs),
  **embarque** le binaire (`include_bytes!`) ; stub vide hors macOS ou sans Xcode.
- **`krello-server`** : axum REST + WebSocket + statique (disque **ou** assets
  embarqués `include_dir`), garde locale (token optionnel), binaire `krello`.
- **`krello-app`** : Tauri v2, serveur in-process port éphémère, fenêtre native,
  icônes icns/ico/png générées, bundle `.app`/`.dmg`/`.msi`.
- **Frontend** : ES modules vanilla servis par le backend — zéro dépendance npm,
  zéro build. Design system CSS (variables, glass, animations), UI en français.

## 3. Modèle de données (SQLite, migration `001_init.sql`)

`boards` (pinned/pinned_at, is_board_template/template_description) · `lists` ·
`cards` (start, due, reminders/reminders_sent JSON, parent_id, recurrence, priority,
is_milestone, duration_days, work_hours, pct_complete, task_type, units, hours_per_day,
constraint_type/date, wbs, cost, cover, time_spent_minutes/tracking_started_at) ·
`labels` · `card_labels` · `checklists` ·
`checklist_items` · `custom_field_defs` · `custom_field_values` · `attachments` ·
`comments` · `activity` · `dependencies` (type, lag_days, UNIQUE) · `schedule_cache`
(es/ef/ls/lf/float/critical) · `baselines` · `baseline_snapshots` · `calendars` ·
`automations` · `automation_runs` (journal d'exécution du bot) · `board_views` · `cards_fts` (FTS5 + triggers) · `comments_fts` (FTS5 external-content + triggers) · `meta`.

Positions **REAL** (insertion par moyenne, renormalisation `0..n` sur collision) ;
index sur `(board_id, archived)`, `(list_id, archived, position)`, `due`, `parent_id`,
partiel reminder-sweep sur `due` ; WAL ;
foreign keys ON ; busy_timeout 5 s ; cache 8 Mo.

## 4. API (extrait, complet dans `docs/API.md`)

`/api/v1/health` · `/boards` CRUD + `/duplicate` `/template` `/board-templates`
(+`/instantiate`) `/activity` `/cards` `/lists`
`/labels` `/fields` `/dependencies` `/schedule` (GET/POST) `/baselines`
`/automations` `/export?format=json|md|csv|ics` `/views` `/stats/global` ·
`/lists/{id}` ·
`/cards/{id}` + `/move` (+`parent_id`) `/duplicate` `/labels` `/cover` `/fields/{fid}`
`/checklists` `/comments` `/attachments` · `/search` (+`include_comments`)
`/import/trello`
`/import/krello` · `/undo` `/redo` · `/ai/*` (status, parse-task, plan,
summarize, priorities, rewrite, checklist) · `/ws` (events JSON).

Erreurs : `400 validation` · `404 not_found` · `409 conflict | dependency_cycle`
· `503 ai_unavailable`.

## 5. Décisions clés & compromis

| Décision | Raison |
|---|---|
| Vanilla JS sans build | fiabilité de compilation hors-ligne, bundle minuscule ; le backend sert tout |
| Helper Swift **process** (pas de FFI) | aucun link Swift dans Rust ; fonctionne CLI **et** Tauri ; isolation mémoire ; testable |
| Undo par snapshots inverses | inverse exact de toute opération y compris suppressions avec enfants |
| Dates schedule ≠ dates cartes | le CPM écrit dans `schedule_cache`, ne mutile jamais les données utilisateur |
| Déplacement inter-board = perte labels/fields | ces données sont board-scopées (comme Trello) |
| WS → refetch ciblé + debounce 150 ms | simplicité + fraîcheur ; resync demandé si lag |
| Bind 127.0.0.1 + garde Host/token | sécurité locale (DNS rebinding) sans friction |

## 6. Plan d'exécution tenu (9 étapes, chacune vérifiée)

1. **Scaffold + schéma** — workspace, migration v1, modèles, 6 tests smoke ✅
2. **Services domaine** — CRUD, positions flottantes + renormalisation, undo/redo,
   labels/checklists/fields/attachments/comments — 11 tests ✅
3. **Scheduler CPM** — calendriers, FS/SS/FF/SF+lag, cycles, types de tâches,
   chemin critique, baselines, WBS — 11 tests ✅
4. **Recherche + import/export + automations** — FTS5+filtres, JSON/MD/CSV/ICS,
   import Trello complet, moteur de règles + sweep — 13 tests ✅
5. **Provider IA** — trait, helper Swift embarqué, use cases JSON validés — 6 tests ✅
   *(statut réel vérifié : `available` sur cette machine)*
6. **Serveur** — axum REST/WS/statique, mapping d'erreurs, 5 tests intégration
   oneshot + **smoke curl live** ✅
7. **Frontend shell** — design system, router, api/ws/dnd, palette ⌘K, Kanban,
   modale carte — **vérifié sous Chromium headless (captures)** ✅
8. **Vues** — calendrier, Gantt (critique, flèches), table, dashboard, IA —
   **vérifié sous Chromium, 0 erreur console** ✅
9. **Tauri + docs** — app native compile (`cargo check -p krello-app`), icônes,
   documentation, CI ✅

## 7. Risques connus

- Apple Intelligence requiert Apple Silicon + macOS 15.1+ + Apple Intelligence
  activée ; sinon l'UI masque l'IA (`/ai/status` → raison machine-lisible).
- Windows : l'IA est désactivée (Foundation Models = macOS) ; tout le reste
  fonctionne (le helper est un stub vide, la crate compile avec MSVC).
- `tauri build` complet nécessite `npm i -g @tauri-apps/cli` (le cœur et
  `cargo check/build` n'en dépendent pas).
