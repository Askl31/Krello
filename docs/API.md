# API Krello — `http://127.0.0.1:<port>/api/v1`

Toutes les réponses sont en JSON (sauf exports). Erreurs :
`{"error": "validation|not_found|conflict|dependency_cycle|ai_unavailable|internal", "message": "…"}`.
Un corps JSON non conforme au schéma du endpoint (champ manquant/typé) est
rejeté **422** par la couche de désérialisation axum (payload typés).

## Santé
| | |
|---|---|
| `GET /health` | `{ok, name, version, boards, ai:{available,backend,reason}}` |

## Boards
| | |
|---|---|
| `GET /boards?include_archived=` | liste |
| `POST /boards` | `{title, description?, background?, calendar_id?, start_date?, deadline?}` |
| `GET/PATCH/DELETE /boards/{id}` | lecture / modif partielle / suppression (cascade, undoable ⌘Z) · `DELETE /boards` → `{deleted: n}` = **tout supprimer** (boards archivés compris, pièces jointes purgées du disque, une seule entrée undo) · `pinned: bool` (épinglés en tête de sidebar) · `position: f64` (**ordre manuel** dans le groupe pinned, drag & drop sidebar) |
| `POST /boards/{id}/duplicate` | copie profonde (conserve le statut modèle s'il y en a un, jamais `pinned`) |
| `PATCH /boards/{id}/template` | `{is_template, template_description?}` — bascule board ↔ **modèle de board** (galerie « Nouveau board ») |
| `GET /board-templates` | galerie des modèles de board (exclus de `GET /boards`) |
| `POST /board-templates/{id}/instantiate` | `{title?}` (défaut = titre du modèle) — instanciation = **copie profonde** (listes, cartes + sous-cartes, labels, champs, dépendances, règles), jamais un modèle ; undo en une entrée |
| `GET /stats/global` | vue d'ensemble cross-board : `{boards, cards_active, overdue, due_7d, done_7d, per_board:[{id,title,active,overdue}], activity:[≤20], urgent:[≤12 cartes priorité urgent/high actives], urgent_total, due_soon:[≤12 cartes échéance aujourd'hui→+2 j], due_soon_total}` |
| `GET /boards/{id}/activity?limit=&card_id=` | flux d'activité · `card_id` → activité de CETTE carte (section « Activité » de la modale) |
| `GET /boards/{id}/lists` `POST` | listes |
| `GET /boards/{id}/cards?include_archived=` `POST` | cartes · création en sous-carte : `parent_id` = carte de la **même liste** (sinon 400) |
| `GET /boards/{id}/cards/meta` | métadonnées par carte en UNE requête : `{card_id: {label_ids, checklists_count, checklist_items, checklist_done, comments_count, attachments_count}}` (hydratation des tuiles sans N+1) |
| `GET /boards/{id}/labels` `POST` | labels |
| `GET /boards/{id}/fields` `POST` | champs personnalisés (définitions) |
| `GET /boards/{id}/dependencies` `POST` | dépendances |
| `GET/POST /boards/{id}/schedule` | résumé CPM `{start, finish, deadline, deadline_met, cards:[{card_id, es, ef, ls, lf, total_float_days, is_critical}]}` (POST = recompute) |
| `GET/POST /boards/{id}/baselines` | baselines `{name}` |
| `GET /boards/{id}/automations` `POST` | règles |
| `GET /boards/{id}/export?format=json|md|csv|ics` | téléchargement |
| `GET /boards/{id}/views` `PUT /views/{view}` | préférences de vue (board, calendar, timeline, table, dashboard) |

## Listes / Cartes
| | |
|---|---|
| `GET/PATCH/DELETE /lists/{id}` · `GET /lists/{id}/count` | count → `{count, wip_exceeded}` |
| `POST /lists/{id}/move` | `{direction: -1\|1}` — échange les positions avec la liste voisine active (gauche/droite dans le Kanban) ; aux bords = no-op ; archivées exclues ; undoable |
| `POST /lists/reorder` | `{board_id, order: [list_id…]}` — **réordonnancement atomique** du DnD : positions finales en une transaction (sans renormalisation intermédiaire), exactement les listes actives du board (sinon 400), undo en une entrée |
| `POST /lists/{id}/duplicate` | duplie la liste : cartes non-archivées avec labels, checklists, champs personnalisés et pièces jointes (**fichiers dupliqués physiquement**) ; dépendances non copiées ; undoable en une opération |
| `GET /cards/{id}` | détail complet : `{card, labels, checklists:[{…,items}], comments, attachments, custom_fields, dependencies_in/out, schedule}` |
| `PATCH /cards/{id}` | tout champ partiel (title, description, start, due, due_complete, priority, is_milestone, duration_days, work_hours, pct_complete, task_type, units, hours_per_day, constraint_type/date, cost, archived…) · `reminders`: `[{"kind":"relative","offset":"15m"\|"30m"\|"1h"\|"2h"\|"1d"\|"2d"\|"1w"} \| {"kind":"absolute","at":"ISO"}]` — **rappels multiples** ; `[]` = aucun ; tout changement de `due`/`reminders` réarme les rappels (offset invalide ou date absurde → 400) · `time_spent_minutes`: **banque un incrément** (+n, jamais un remplacement) · `tracking_started_at`: `"ISO datetime"` = session de suivi en cours, **`""` = arrêter (banque l'écart depuis le début)** |
| `DELETE /cards/{id}` | suppression (undoable) |
| `POST /cards/{id}/move` | `{list_id?, parent_id?, position? \| before? \| after?}` (cross-board ok) · `parent_id`: `""` = top-level, `"card_x"` = sous-carte de cette carte (**même liste requise**, cycle → 400), absent = inchangé (même liste) ou détaché (autre liste) ; anchor `before`/`after` doit être au même niveau ; le positionnement se calcule parmi les frères du même parent |
| `POST /cards/{id}/duplicate` | copie + enfants |
| `PATCH /cards/{id}/template` | `{is_template, template_description?}` — bascule carte ↔ modèle (badge, aide galerie) |
| `POST /cards/{id}/instantiate` | `{list_id, name?, keep?, at?: "top"\|"bottom", position?}` — instancie une carte depuis un modèle ; copie profonde en une transaction : titre + description toujours, puis selon `keep` (défaut tout) : `checklists` (items décochés, ids neufs), `labels`, `attachments` (octets dupliqués), `customFields`, `cover` ; jamais copiés : dates, `due_complete`/`pct_complete`, commentaires, activité. 404 si modèle/liste introuvable, 400 si non-modèle ou liste hors board |
| `GET /boards/{id}/templates` | galerie : cartes `is_template=1` non archivées (ordre = position dans leur liste) |
| `PUT /cards/{id}/labels` | `[label_id…]` (remplace) |
| `POST/DELETE /cards/{id}/labels/{label_id}` | attache/détache |
| `POST /cards/{id}/cover` | `{color: "#hex" \| null}` |
| `PUT /cards/{id}/fields/{field_id}` | `{value}` ("" = efface) |
| `GET/POST /cards/{id}/checklists` · `DELETE /checklists/{id}` | checklists |
| `POST /checklists/{id}/items` · `PATCH/DELETE /checklist-items/{id}` | éléments |
| `GET/POST /cards/{id}/comments` · `DELETE /comments/{id}` | commentaires |
| `GET/POST /cards/{id}/attachments` (multipart `file`) · `GET /attachments/{id}/download` · `DELETE` · `POST /attachments/{id}/cover` | pièces jointes |

## Dépendances (MS Project)
`POST /boards/{id}/dependencies` `{predecessor_id, successor_id, type: "FS|SS|FF|SF", lag_days?}` —
`409 dependency_cycle` si cycle (message = chemin). `PATCH/DELETE /dependencies/{id}`.

## Baselines
`POST /boards/{id}/baselines {name}` · `DELETE /baselines/{id}` ·
`GET /baselines/{id}/compare` → `[{card_id, title, start_baseline, start_now, due_baseline, due_now, duration_*, work_*, delta_due_days}]`.

## Automatisations (voir `docs/AUTOMATIONS.md` pour le DSL complet)
`POST /boards/{id}/automations` `{name, trigger, conditions?, actions?, enabled?}` ·
`PATCH/DELETE /automations/{id}` · `POST /automations/{id}/run` → `{cards_touched}` ·
`GET /automations/{id}/runs?limit=50` · `GET /boards/{id}/automations/runs?limit=100` (journal `automation_runs`).
Déclencheurs : `card_created, card_moved, card_labeled, card_unlabeled, card_archived, card_commented, checklist_item_checked, card_due_soon, card_due, button, scheduled_daily, scheduled_weekly, scheduled_monthly`.
Actions cross-board possibles (`list_id` de n'importe quel board) : `move, copy, create_card, label, unlabel, due, clear_due, priority, check, check_item, archive, unarchive, comment, set_title, set_field` — avec variables `{{card.title}}`, `{{list.title}}`, `{{board.title}}`, `{{date}}`…

## Calendriers
`GET/POST /calendars` `{name, weekends?: "[6,0]", holidays?: "[\"2027-01-01\"]", hours_per_day?}` · `PATCH/DELETE /calendars/{id}`.

## Réglages — sauvegardes automatiques
| | |
|---|---|
| `GET /settings/backup` | `{backup_dir, interval_hours, keep, last_backup_at, last_backup_file}` (défauts : dir null = désactivé, 24 h, 10 copies) |
| `PUT /settings/backup` | `{backup_dir? ("": désactive), interval_hours? (1..720), keep? (1..100)}` — répertoire créé si besoin, 400 si inutilisable |
| `POST /settings/backup/run` | sauvegarde immédiate → `{file, size}` ; 400 sans répertoire configuré |
| `POST /settings/backup/pick` | ouvre le **sélecteur natif** de répertoire (Finder/Explorer) → `{dir: "…"\|null}` ; **400 hors app native** (fenêtre uniquement, mode navigateur → saisie manuelle) |

Mécanique : `VACUUM INTO` → `krello-backup-<YYYY-MM-DD-HHMMSS>.db` (base complète
compacte, ouvrable telle quelle) ; rotation automatique au-delà de `keep`.
Le serveur vérifie chaque minute : sauvegarde dès que `interval_hours` est écoulé.
Rien n'est undoable ni journalisé dans `activity` (réglages globaux).

## Système & quick-add macOS (Spotlight HUD) — voir `docs/MACOS_SIRI_SPOTLIGHT.md`
| | |
|---|---|
| `GET /system` | `{platform: "macos"\|"windows"\|"linux", native_app, exe, version}` — pilote l'affichage de l'onglet macOS |
| `POST /deep-link` | `{url: "krello://task/new?list=…&board=…&title=…&confirm=1"}` → `{kind:"created", card, list, board}` ou `{kind:"needs_input", board, list, title}` ; 400 scheme/action invalide, 404 liste introuvable. |
| `GET /macos/quick-add/status` | `{supported: bool, installed: bool, path?: string, app_name: string}` — vérifie si l'applet `Ajouter à Krello.app` est installée dans `~/Applications/` |
| `POST /macos/quick-add/install` | compile `quick_add.swift` via `swiftc -O`, génère et signe le bundle `Ajouter à Krello.app` (`LSUIElement = true`) dans `~/Applications/`, et l'enregistre dans LaunchServices pour Spotlight → `{installed: true, path}` ; **400 hors macOS** |
| `POST /macos/quick-add/uninstall` | supprime `Ajouter à Krello.app` et nettoie LaunchServices → `{installed: false}` ; **400 hors macOS** |
| `POST /macos/quick-add/test` | lance `Ajouter à Krello.app` avec `open` → `{success: true}` ; **400 hors macOS**, **404 si non installé** |

CLI (binaire `krello` et binaire de l'app) : `task add --title "…" [--list "…"] [--board "…"] [--db PATH]`
(`--title -` = stdin) → écrit la carte directement dans la base (WAL, coexiste avec l'app ouverte), imprime `OK …`, code 0/1/2.

Deep link enregistré sur macOS uniquement (Tauri `krello://`, plugin deep-link) ; Windows inchangé.

## Recherche & import
`GET /search?q=&board_id=&list_id=&label_id=&priority=&due_before=&due_after=&has_due=&include_archived=&include_comments=` (FTS5 + filtres ; `include_comments=true` par défaut : les cartes dont un **commentaire** matche sont retournées ; archivées incluses sur `include_archived=1`)
`POST /import/trello` (body = JSON export Trello) · `POST /import/krello` (round-trip export, `parent_id` des sous-cartes remappé).

## Undo / Redo
`POST /undo` · `POST /redo` → `{undone|redone: label|null}` · `GET /undo` (labels) · `DELETE /undo` (clear).

## IA (Apple Intelligence, on-device)
| | |
|---|---|
| `GET /ai/status` | `{available, backend, reason?}` |
| `POST /ai/parse-task` | `{input, list_id, create?}` → carte NL → champs (+création) |
| `POST /ai/plan` | `{goal, board_id, list_id, apply?}` → tâches + dépendances FS + schedule |
| `POST /ai/summarize` | `{board_id}` → résumé 3 phrases |
| `POST /ai/priorities` | `{board_id, apply?}` → suggestions `{card_id, priority, reason}` |
| `POST /ai/rewrite` | `{text}` |
| `POST /ai/checklist` | `{card_id, apply?}` → items de checklist |

## WebSocket
`GET /ws` — événements serveur : `BoardCreated/Updated/Deleted`, `ListCreated/Updated/Deleted`,
`CardCreated/Updated/Moved/Deleted`, `LabelsUpdated`, `ChecklistsUpdated`, `CommentsUpdated`,
`AttachmentsUpdated`, `FieldsUpdated`, `DependenciesUpdated`, `ScheduleUpdated`,
`AutomationsUpdated`, `BaselinesUpdated`, `CalendarsUpdated`, `UndoApplied`, `resync`,
`ReminderDue` (rappel déclenché par le sweep : `{board_id, card_id, title, due}` — une seule
fois par **(rappel, échéance)** ; réémis si `due` ou `reminders` change).
Format : `{"type": "...", "data": {...}}`.
