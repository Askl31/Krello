# API Krello — `http://127.0.0.1:<port>/api/v1`

Toutes les réponses sont en JSON (sauf exports). Erreurs :
`{"error": "validation|not_found|conflict|dependency_cycle|ai_unavailable|internal", "message": "…"}`.

## Santé
| | |
|---|---|
| `GET /health` | `{ok, name, version, boards, ai:{available,backend,reason}}` |

## Boards
| | |
|---|---|
| `GET /boards?include_archived=` | liste |
| `POST /boards` | `{title, description?, background?, calendar_id?, start_date?, deadline?}` |
| `GET/PATCH/DELETE /boards/{id}` | lecture / modif partielle / suppression (cascade) |
| `POST /boards/{id}/duplicate` | copie profonde |
| `GET /boards/{id}/activity?limit=` | flux d'activité |
| `GET /boards/{id}/lists` `POST` | listes |
| `GET /boards/{id}/cards?include_archived=` `POST` | cartes |
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
| `GET /cards/{id}` | détail complet : `{card, labels, checklists:[{…,items}], comments, attachments, custom_fields, dependencies_in/out, schedule}` |
| `PATCH /cards/{id}` | tout champ partiel (title, description, start, due, due_complete, priority, is_milestone, duration_days, work_hours, pct_complete, task_type, units, hours_per_day, constraint_type/date, cost, archived…) |
| `DELETE /cards/{id}` | suppression (undoable) |
| `POST /cards/{id}/move` | `{list_id?, position? \| before? \| after?}` (cross-board ok) |
| `POST /cards/{id}/duplicate` | copie + enfants |
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

## Automatisations
`POST /boards/{id}/automations` `{name, trigger, conditions?, actions?, enabled?}` ·
`PATCH/DELETE /automations/{id}` · `POST /automations/{id}/run` → `{cards_touched}`.
Conditions : `{"list_id","from_list_id","to_list_id","label_id","title_contains","days"}`.
Actions : `[{"type":"move","list_id"},{"type":"label","label_id"},{"type":"due","offset_days","time"},{"type":"priority","value"},{"type":"check","item_title"},{"type":"archive"}]`.

## Calendriers
`GET/POST /calendars` `{name, weekends?: "[6,0]", holidays?: "[\"2027-01-01\"]", hours_per_day?}` · `PATCH/DELETE /calendars/{id}`.

## Recherche & import
`GET /search?q=&board_id=&list_id=&label_id=&priority=&due_before=&due_after=&has_due=` (FTS5 + filtres)
`POST /import/trello` (body = JSON export Trello) · `POST /import/krello` (round-trip export).

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
`AutomationsUpdated`, `BaselinesUpdated`, `CalendarsUpdated`, `UndoApplied`, `resync`.
Format : `{"type": "...", "data": {...}}`.
