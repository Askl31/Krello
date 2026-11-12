# Intégration macOS — ajout rapide via Spotlight & Siri

## Objectif

Depuis **Spotlight** (`⌘Espace` → taper le nom du raccourci → Entrée) ou **Siri**
(phrase personnalisée, ex. « Nouvelle tâche Krello »), enregistrer en quelques
secondes une nouvelle tâche **dans une liste déterminée**, sans ouvrir Krello
à la main. La saisie du titre se fait au clavier (Spotlight) ou à la voix
(Siri, dictée dans le dialogue du raccourci).

**Windows reste inchangé** : tout le code natif macOS est compilé/géré
uniquement pour la cible macOS (voir §6).

## Pourquoi cette architecture (décision clé)

Sur macOS, Siri et Spotlight ne peuvent invoquer que des **raccourcis de l'app
Raccourcis** (ou des App Intents natifs, réservés aux apps Swift — hors portée
d'un app Tauri). Un raccourci est :

- **indexé par Spotlight** (⌘Espace le trouve par son nom) ;
- **exposable à Siri** (« Ajouter à Siri » dans l'app Raccourcis, phrase au choix).

Un raccourci ne peut pas appeler directement notre logique Rust : il passe par
un **deep link `krello://`** (scheme enregistré par l'app Tauri) ou, en
repli, par le **binaire CLI** embarqué. D'où les 3 composants :

```
Spotlight / Siri
      │  (lance le raccourci .shortcut généré par Krello)
      ▼
Raccourci « Krello · <Liste> »  =  [Demander la saisie] → [Exécuter un script shell]
      │                                    │ stdin = texte saisi/dicté
      ▼                                    ▼
écrit <data>/siri-inbox.txt      open "krello://task/new?list=<liste>"
                                 (repli : "<binaire>" task add --title … --list …)
                                          │
                                          ▼
                        app Tauri (deep-link) → frontend → POST /api/v1/deep-link
                                          │
                                          ▼
                        Service::handle_deep_link → create_card (WS → UI live)
                        ou needs_input → modale pré-remplie dans Krello
```

## Composants

### 1. `krello-core` — logique métier (testable, cross-platform)

- **`src/deeplink.rs`**
  - `parse_deep_link(url) -> DeepLink` : parse `krello://task/new?list=…&board=…&title=…&confirm=1`
    (décodage percent-encoding maison, sans nouvelle dépendance).
  - `Service::find_list(list_ref, board_ref) -> (List, Board)` : par id d'abord,
    sinon par nom (insensible à la casse), filtré par board si fourni, sinon
    premier board par `created_at`.
  - `Service::data_dir()` (= parent de `attachments_dir`) et
    `siri_inbox_path()` = `<data_dir>/siri-inbox.txt`.
  - `Service::handle_deep_link(url) -> DeepLinkOutcome` :
    `Created { card, list, board }` ou `NeedsInput { board, list, title }`
    (titre vide ou `confirm=1`). Le titre vient du paramètre `title` de l'URL,
    sinon du fichier inbox (lu puis supprimé). Création via `create_card`
    (événements WS + undo + FTS habituels).
- **`src/shortcut.rs`** — génération des raccourcis (plist binaire, crate `plist`) :
  - `task_add_script(list_name, exe)` : script shell du raccourci (inbox +
    `open krello://…` + repli CLI), avec échappement shell correct.
  - `build_shortcut(name, script) -> Vec<u8>` : plist binaire avec 2 actions —
    `is.workflow.actions.ask` (`WFAskActionPrompt`, `WFInputType=Text`,
    `WFAskActionDefaultAnswer`) puis `is.workflow.actions.runshellscript`
    (`WFShellScript`, `WFShellScriptShell=/bin/sh`, `WFInput="to stdin"`).
  - `write_pack(dir, targets, exe) -> Vec<PathBuf>` : un `.shortcut` par liste
    (`Krello · <Liste>.shortcut`, nom de fichier assaini) + `LISEZMOI.txt`
    (import : double-clic → « Ajouter le raccourci » ; Siri : « Ajouter à
    Siri » ; Spotlight : ⌘Espace + nom).

### 2. `krello-server` — API + CLI

- Routes :
  - `GET /api/v1/system` → `{ platform, native_app, exe, version }` (le
    frontend s'en sert pour afficher/masquer la section macOS).
  - `POST /api/v1/deep-link` `{ url }` → `handle_deep_link` → JSON
    (`created` + carte, ou `needs_input` + board/liste/titre).
  - `POST /api/v1/macos/siri-pack` `{ dir }` → génère le pack (macOS
    uniquement, sinon erreur 400 explicite) → `{ files: [...] }`.
  - `POST /api/v1/macos/siri-pack/pick` → sélecteur de dossier natif (rfd,
    seulement si `KRELLO_NATIVE_DIALOG=1`, même pattern que
    `/settings/backup/pick`).
- **CLI** `krello task add --title "…" [--list "…"] [--board "…"] [--db …]`
  (`--title -` = stdin) : fonction partagée `cli_task_add` dans la lib
  serveur, interceptée **avant** le démarrage du serveur dans
  `crates/krello-server/src/main.rs` **et** dans `src-tauri/src/main.rs`
  (même base par défaut `~/Library/Application Support/com.krello.app/krello.db`
  sur macOS). Écrit directement dans SQLite (WAL : coexiste avec l'app
  ouverte) et imprime la carte créée.

### 3. `src-tauri` — app native (macOS uniquement)

- `tauri-plugin-deep-link` en dépendance `cfg(target_os = "macos")` +
  enregistrement du plugin sous `#[cfg(target_os = "macos")]`.
- `src-tauri/tauri.macos.conf.json` (fusionné automatiquement sur macOS
  uniquement) : `app.withGlobalTauri = true` (accès à
  `window.__TAURI__.deepLink` depuis le frontend sans bundler) +
  `plugins.deep-link.desktop.schemes = ["krello"]` (génère le
  `CFBundleURLTypes` du bundle ; l'URL de lancement à froid est délivrée via
  `getCurrent()`).
- `src-tauri/capabilities/macos.json` (`platforms: ["macOS"]`) :
  `core:default` + `deep-link:default`.
- Interception `task add` avant le builder Tauri (même helper que le serveur).

### 4. Frontend (inchangé en mode navigateur / Windows)

- `frontend/src/deeplink.js` : si `window.__TAURI__?.deepLink` existe,
  `getCurrent()` + `onOpenUrl()` → `POST /api/v1/deep-link` pour chaque URL →
  toast « ✓ carte ajoutée à <liste> » (le WS met la vue à jour) ou modale
  pré-remplie (`needs_input`) → `createCard`.
- Onglet **Paramètres → macOS** (visible si `/api/v1/system` dit
  `platform === "macos"`) : bouton « Générer les raccourcis Siri/Spotlight »
  (sélecteur natif dans l'app, saisie manuelle en navigateur) + liste des
  listes avec commande CLI copiable (`<exe> task add --title "…" --list "…"`).
- `api.js` : `system()`, `deepLink(url)`, `siriPack(dir)`, `pickSiriPackDir()`.

### 5. Fichiers générés (pack)

```
<dossier choisi>/
  Krello · <Liste 1>.shortcut      ← un raccourci par liste (tous boards)
  Krello · <Liste 2>.shortcut
  …
  LISEZMOI.txt                     ← procédure d'import + Siri + Spotlight
```

Chaque `.shortcut` : « Demander la saisie » → « Exécuter un script shell » :

```sh
TITLE=$(cat)
[ -z "$TITLE" ] && exit 0
INBOX="$HOME/Library/Application Support/com.krello.app/siri-inbox.txt"
mkdir -p "$(dirname "$INBOX")"
printf '%s' "$TITLE" > "$INBOX" || exit 1
open "krello://task/new?list=<liste url-encodée>" 2>/dev/null && exit 0
"<chemin binaire>" task add --title "$TITLE" --list "<liste>"
rm -f "$INBOX"
```

## Sécurité & limites

- Le serveur écoute toujours sur `127.0.0.1` uniquement + garde `Host`
  loopback ; `/api/v1/deep-link` hérite de ces gardes.
- Le fichier inbox est local (`~/Library/Application Support/...`), lu puis
  supprimé à la consommation ; un titre passé en clair dans l'URL est aussi
  accepté (`?title=`) pour les scripts.
- Siri « Ajoute X à Krello » en langage libre (App Intents NLP) n'est pas
  atteignable depuis Tauri : on passe par un raccourci + phrase Siri, la liste
  étant fixée par le raccourci (un par liste, renommable/dupliable dans
  Raccourcis).
- Spotlight indexe les raccourcis importés : aucune autre intégration
  (mdimporter) n'est nécessaire pour l'ajout rapide.
- Hors macOS, `POST /api/v1/macos/siri-pack` → 400 « macOS uniquement » ;
  l'onglet est masqué ; le deep link n'existe pas (pas de plugin enregistré).

## Tests

- Core : parse deep link (query, encodage, confirm), `handle_deep_link`
  (création, needs_input, inbox), `build_shortcut` (re-parse du plist,
  structure des 2 actions), `write_pack` (fichiers écrits).
- Serveur : endpoints HTTP via `test_krello_app` (`/system`, `/deep-link`
  created + needs_input + liste inconnue, `/macos/siri-pack` + refus hors
  macOS), `cli_task_add` sur DB temporaire.
- Sur la machine : `plutil -lint` + `shortcuts sign --mode anyone` sur un
  `.shortcut` généré ; E2E CLI `task add` contre une DB temporaire avec le
  serveur lancé (concurrence WAL).
