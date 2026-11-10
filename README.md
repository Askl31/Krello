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
- **Gantt / Timeline (MS Project)** : durées en jours ouvrés, charge, % accompli,
  jalons, dépendances **FS/SS/FF/SF + lag**, **chemin critique** (CPM complet :
  marges, ES/EF/LS/LF), calendriers avec jours fériés, baselines et dérives, WBS,
  zoom jour/semaine/mois, drag pour replanifier.
- **Calendrier** mensuel (drag pour replanifier), **Table** triable éditable,
  **Dashboard** (stats, graphiques, baselines, résumé IA).
- **Automatisations** locales (Butler-like) : déclencheurs, conditions, actions,
  sweep planifié.
- **Undo/redo ⌘Z** (100 opérations), **palette ⌘K**, thèmes clair/sombre,
  WebSocket temps réel, tout en français.
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
./target/release/krello --help          # --db --port --token --frontend --open

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

`⌘K` palette · `⌘Z` / `⇧⌘Z` undo/redo · `Échap` ferme modales ·
Entrée valide quick-add · glisser une carte du Gantt ou du calendrier replanifie.

## Tests

53 tests (smoke, cartes/déplacements/undo, CPM+calendriers+baselines, recherche,
import/export/automations, IA, intégration HTTP) : `cargo test --workspace`.
