# Automatisations — moteur « bot » local (façon Butler)

Krello embarque un moteur de règles **100 % local** inspiré de Butler (Trello).
Une règle vit sur un **board** mais ses actions peuvent cibler **n'importe quel
autre board** (copier / créer / déplacer des cartes). Chaque exécution est
journalisée dans la table `automation_runs` (audit + dédup des déclencheurs
planifiés) et visible dans l'UI : *Paramètres du board → Automatisations →
Journal d'exécution*.

- Moteur : `crates/krello-core/src/service/automations.rs`
- Schéma : migration `003_automations_v3.sql` (table `automation_runs`)
- UI : constructeur de règles dans `frontend/src/views/settings.js`
- Balayage planifié : toutes les minutes côté serveur (`run_scheduled_sweep`)

## 1. Structure d'une règle

```json
{
  "id": "auto_…",
  "board_id": "board_…",
  "name": "Créer un suivi dans « Suivi »",
  "enabled": true,
  "trigger": "card_moved",
  "conditions": "{\"to_list_id\": \"list_…\"}",
  "actions": "[{\"type\":\"create_card\",\"list_id\":\"list_…\",\"title\":\"Suivi de {{card.title}}\"}]"
}
```

`conditions` et `actions` sont des chaînes JSON (objet / tableau).

## 2. Déclencheurs (13)

| Trigger | Catégorie | Quand |
|---|---|---|
| `card_created` | événement | carte créée (y compris copie/création par une autre règle) |
| `card_moved` | événement | carte déplacée (liste ou board ; tiré sur les deux boards en cross-board) |
| `card_labeled` | événement | label ajouté à la carte |
| `card_unlabeled` | événement | label retiré |
| `card_archived` | événement | carte archivée |
| `card_commented` | événement | commentaire ajouté |
| `checklist_item_checked` | événement | un élément de checklist passe à coché |
| `card_due_soon` | balayage | échéance dans `0..days` jours (défaut 1), une fois/jour/carte |
| `card_due` | balayage | échéance atteinte ou dépassée, non terminée, une fois/jour/carte |
| `button` | manuel | bouton « ▶ Lancer » (cibles = conditions) |
| `scheduled_daily` | balayage | chaque jour à `time` (défaut 00:00), une fois/jour |
| `scheduled_weekly` | balayage | chaque `weekday` (1=lun..7=dim, défaut 1) à `time` |
| `scheduled_monthly` | balayage | chaque `day` du mois (1..31, défaut 1) à `time` |

## 3. Conditions (objet JSON, toutes optionnelles, cumulatives « ET »)

| Clé | Type | Signification |
|---|---|---|
| `list_id` | string | la carte est dans cette liste (ou `to_list_id` pour `card_moved`) |
| `list_ids` | [string] | la carte est dans l'une de ces listes |
| `from_list_id` / `to_list_id` | string | déplacement depuis/vers cette liste (`card_moved`) |
| `label_id` | string | la carte a ce label |
| `without_label_id` | string | la carte n'a PAS ce label |
| `title_contains` | string | le titre contient (insensible à la casse) |
| `description_contains` | string | la description contient |
| `priority` | string | `none\|low\|medium\|high\|urgent` |
| `has_due` | bool | avec / sans échéance |
| `overdue` | bool | échéance atteinte/passée et non terminée |
| `days` | number | `card_due_soon` : fenêtre en jours (défaut 1) |
| `time` | `"HH:MM"` | planifiées : heure de déclenchement |
| `weekday` | 1..7 | `scheduled_weekly` : jour (ISO, 1=lundi) |
| `day` | 1..31 | `scheduled_monthly` : jour du mois |

## 4. Actions (15)

| Action | Champs | Notes |
|---|---|---|
| `move` | `list_id`, `position?` (`"top"` ou nombre) | **cross-board** : supprime labels/champs du board source (comme un déplacement manuel) |
| `copy` | `list_id`, `title?`, `keep?{labels,checklists,description,cover,fields}` | **cross-board** : labels recréés par (nom, couleur) sur le board cible, checklists copiées en profondeur ; **les dates ne sont jamais copiées** |
| `create_card` | `list_id`, `title` (requis), `description?`, `priority?` | **crée une carte neuve dans n'importe quel board** |
| `label` / `unlabel` | `label_id` | label du board de la carte |
| `due` | `offset_days?` (défaut 0) ou `date`, `time?` | |
| `clear_due` | — | supprime l'échéance |
| `priority` | `value` | |
| `check` | `item_title?` | ajoute un élément non coché à la 1ʳᵉ checklist (créée si besoin) |
| `check_item` | `item_title` | coche l'élément existant de ce titre |
| `archive` / `unarchive` | — | |
| `comment` | `text` | commentaire (avec variables) |
| `set_title` | `value` | renomme (avec variables) |
| `set_field` | `field_id`, `value` | champ du board de la carte |

### Variables de template
Dans `title`, `text`, `value` (actions `copy`, `create_card`, `comment`, `set_title`) :
`{{card.title}}`, `{{card.id}}`, `{{card.description}}`, `{{card.priority}}`,
`{{list.title}}`, `{{board.title}}`, `{{trigger}}`, `{{label.name}}`,
`{{date}}` (aujourd'hui), `{{now}}`.

## 5. Cross-board & sécurité

- Les actions `copy` / `create_card` / `move` acceptent une `list_id` de
  **n'importe quel board** : le board cible est déduit de la liste.
- **Cascade** : une carte créée/déplacée par une règle déclenche les règles
  `card_created` / `card_moved` du board cible — profondeur limitée à **4**
  (garde-fou anti-boucle).
- Les actions du bot s'appliquent en SQL direct : elles ne sont **pas**
  annulables par ⌘Z (comme avant), mais chaque exécution est journalisée
  (`automation_runs`, conservé 500 entrées par règle) et l'activité du board
  reçoit une entrée `automation`.
- Déplacement cross-board = perte des labels/champs (données board-scopées),
  identique au déplacement manuel.

## 6. Exemples

### « Quand une carte entre dans Terminé, créer un suivi dans un autre board »
```json
{
  "name": "Suivi cross-board",
  "trigger": "card_moved",
  "conditions": "{\"to_list_id\": \"list_termine\"}",
  "actions": "[{\"type\":\"create_card\",\"list_id\":\"list_suivi_autre_board\",\"title\":\"Suivi de {{card.title}} ({{board.title}})\",\"priority\":\"high\"}]"
}
```

### « Copier la carte (labels + checklists) vers le board Archive quand archivée »
```json
{
  "trigger": "card_archived",
  "conditions": "{}",
  "actions": "[{\"type\":\"copy\",\"list_id\":\"list_archive\",\"title\":\"{{card.title}} (archivée le {{date}})\"}]"
}
```

### « Toutes les cartes urgentes de la liste Revue passent en haut, chaque lundi 9h »
```json
{
  "trigger": "scheduled_weekly",
  "conditions": "{\"list_id\":\"list_revue\",\"priority\":\"urgent\",\"time\":\"09:00\",\"weekday\":1}",
  "actions": "[{\"type\":\"move\",\"list_id\":\"list_revue\",\"position\":\"top\"},{\"type\":\"comment\",\"text\":\"Revue hebdo du {{date}}\"}]"
}
```

### « Échéance dépassée → label rouge + commentaire »
```json
{
  "trigger": "card_due",
  "conditions": "{}",
  "actions": "[{\"type\":\"label\",\"label_id\":\"lbl_retard\"},{\"type\":\"comment\",\"text\":\"⚠ Échéance dépassée — à traiter ({{date}})\"}]"
}
```

## 7. API

```
GET    /api/v1/boards/{id}/automations            liste des règles
POST   /api/v1/boards/{id}/automations            {name, trigger, conditions?, actions?, enabled?}
PATCH  /api/v1/automations/{id}                   {name?, enabled?, trigger?, conditions?, actions?}
DELETE /api/v1/automations/{id}
POST   /api/v1/automations/{id}/run               exécution manuelle → {cards_touched}
GET    /api/v1/automations/{id}/runs?limit=50     journal de la règle
GET    /api/v1/boards/{id}/automations/runs?limit=100   journal du board
```

Erreurs : `400` si trigger/action/conditions invalide (liste des valeurs
attendue dans le message).

Événement WebSocket : `AutomationRan { board_id }` émis à chaque exécution.
