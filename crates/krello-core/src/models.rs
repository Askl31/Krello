//! Domain models mirroring the SQLite schema. All persisted dates are ISO-8601 TEXT.

use serde::{Deserialize, Serialize};

pub const PRIORITIES: [&str; 5] = ["none", "low", "medium", "high", "urgent"];
pub const TASK_TYPES: [&str; 3] = ["fixed_duration", "fixed_work", "fixed_units"];
pub const CONSTRAINT_TYPES: [&str; 3] = ["asap", "deadline", "start_not_before"];
pub const DEP_TYPES: [&str; 4] = ["FS", "SS", "FF", "SF"];
pub const VIEWS: [&str; 5] = ["board", "calendar", "timeline", "table", "dashboard"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Board {
    pub id: String,
    pub title: String,
    pub description: String,
    /// JSON: {"type":"color"|"gradient","value":"..."}
    pub background: String,
    pub calendar_id: Option<String>,
    pub start_date: Option<String>,
    pub deadline: Option<String>,
    /// Free-form JSON settings (default view, wip enforcement, templates...)
    pub settings: String,
    pub archived: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct List {
    pub id: String,
    pub board_id: String,
    pub title: String,
    pub position: f64,
    #[serde(default)]
    pub wip_limit: Option<i64>,
    #[serde(default)]
    pub archived: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Card {
    pub id: String,
    pub board_id: String,
    pub list_id: String,
    pub title: String,
    pub description: String,
    pub position: f64,
    #[serde(default)]
    pub archived: bool,
    /// Template flag: template cards are hidden from work views/search/counters
    /// and serve as deep-copy sources for instantiation.
    #[serde(default)]
    pub is_template: bool,
    /// Optional helper text shown in the template gallery.
    #[serde(default)]
    pub template_description: Option<String>,
    /// JSON cover: {"type":"color","value":"#hex"} | {"type":"attachment","value":id} | null
    pub cover: Option<String>,
    pub start: Option<String>,
    pub due: Option<String>,
    #[serde(default)]
    pub due_complete: bool,
    /// "15m"|"1h"|"1d"|"2d" or ISO datetime
    pub reminder: Option<String>,
    /// JSON recurrence {"freq":"daily|weekly|monthly","interval":n,"until":"date"}
    pub recurrence: Option<String>,
    #[serde(default)]
    pub priority: String,
    #[serde(default)]
    pub is_milestone: bool,
    /// Working days; None = derived from start/due.
    pub duration_days: Option<f64>,
    pub work_hours: Option<f64>,
    /// 0..=100
    #[serde(default)]
    pub pct_complete: i64,
    #[serde(default)]
    pub task_type: String,
    #[serde(default)]
    pub units: f64,
    #[serde(default)]
    pub hours_per_day: f64,
    #[serde(default)]
    pub constraint_type: String,
    pub constraint_date: Option<String>,
    #[serde(default)]
    pub wbs: String,
    #[serde(default)]
    pub cost: f64,
    pub created_at: String,
    pub updated_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Label {
    pub id: String,
    pub board_id: String,
    pub name: String,
    pub color: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checklist {
    pub id: String,
    pub card_id: String,
    pub title: String,
    pub position: f64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecklistItem {
    pub id: String,
    pub checklist_id: String,
    pub title: String,
    pub checked: bool,
    pub position: f64,
    pub due: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomFieldDef {
    pub id: String,
    pub board_id: String,
    pub name: String,
    /// text|number|date|checkbox|select
    #[serde(rename = "type")]
    pub kind: String,
    /// JSON array for select options
    pub options: String,
    pub position: f64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomFieldValue {
    pub card_id: String,
    pub field_id: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    pub id: String,
    pub card_id: String,
    pub filename: String,
    pub stored_path: String,
    pub mime: String,
    pub size: i64,
    pub is_cover: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comment {
    pub id: String,
    pub card_id: String,
    pub body: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Activity {
    pub id: String,
    pub board_id: String,
    pub card_id: Option<String>,
    pub kind: String,
    pub payload: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    pub id: String,
    pub board_id: String,
    pub predecessor_id: String,
    pub successor_id: String,
    /// FS|SS|FF|SF
    #[serde(rename = "type")]
    pub kind: String,
    pub lag_days: f64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleEntry {
    pub card_id: String,
    pub es: Option<String>,
    pub ef: Option<String>,
    pub ls: Option<String>,
    pub lf: Option<String>,
    pub total_float_days: Option<f64>,
    pub is_critical: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Baseline {
    pub id: String,
    pub board_id: String,
    pub name: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaselineSnapshot {
    pub baseline_id: String,
    pub card_id: String,
    pub start: Option<String>,
    pub due: Option<String>,
    pub duration_days: Option<f64>,
    pub work_hours: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Calendar {
    pub id: String,
    pub name: String,
    /// JSON [0..6], 0=Sunday
    pub weekends: String,
    /// JSON ["YYYY-MM-DD"]
    pub holidays: String,
    pub hours_per_day: f64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Automation {
    pub id: String,
    pub board_id: String,
    pub name: String,
    pub enabled: bool,
    /// card_created|card_moved|card_labeled|card_due_soon|button|scheduled_daily|scheduled_weekly
    pub trigger: String,
    /// JSON conditions {"list_id":..,"from_list_id":..,"to_list_id":..,"label_id":..,"days":..}
    pub conditions: String,
    /// JSON actions [{"type":"move","list_id":..},{"type":"label","label_id":..},{"type":"due","offset_days":n,"time":"18:00"},{"type":"check","item_title":..},{"type":"priority","value":..},{"type":"archive"}]
    pub actions: String,
    pub created_at: String,
}

/// One execution of an automation rule (audit log; also used to dedupe scheduled triggers per day).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationRun {
    pub id: String,
    pub automation_id: String,
    /// Board owning the rule.
    pub board_id: String,
    pub trigger: String,
    /// Target card, NULL for board-level runs.
    pub card_id: Option<String>,
    pub ok: bool,
    /// JSON detail: applied actions or error message.
    pub detail: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardView {
    pub board_id: String,
    /// board|calendar|timeline|table|dashboard
    #[serde(rename = "view")]
    pub view: String,
    pub settings: String,
}

// ---------- request payloads ----------

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct CreateBoard {
    pub title: String,
    pub description: String,
    pub background: Option<String>,
    pub calendar_id: Option<String>,
    pub start_date: Option<String>,
    pub deadline: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct UpdateBoard {
    pub title: Option<String>,
    pub description: Option<String>,
    pub background: Option<String>,
    pub calendar_id: Option<String>,
    pub start_date: Option<String>,
    pub deadline: Option<String>,
    pub settings: Option<String>,
    pub archived: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct CreateList {
    pub title: String,
    pub wip_limit: Option<i64>,
    pub position: Option<f64>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct UpdateList {
    pub title: Option<String>,
    pub wip_limit: Option<i64>,
    pub position: Option<f64>,
    pub archived: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct CreateCard {
    pub title: String,
    pub description: String,
    pub list_id: String,
    pub position: Option<f64>,
    pub cover: Option<String>,
    pub start: Option<String>,
    pub due: Option<String>,
    pub due_complete: Option<bool>,
    pub reminder: Option<String>,
    pub recurrence: Option<String>,
    pub priority: Option<String>,
    pub is_milestone: Option<bool>,
    pub duration_days: Option<f64>,
    pub work_hours: Option<f64>,
    pub pct_complete: Option<i64>,
    pub task_type: Option<String>,
    pub units: Option<f64>,
    pub hours_per_day: Option<f64>,
    pub constraint_type: Option<String>,
    pub constraint_date: Option<String>,
    pub cost: Option<f64>,
    pub label_ids: Option<Vec<String>>,
    /// Custom fields: {field_id: value}
    pub custom_fields: Option<serde_json::Map<String, serde_json::Value>>,
    pub checklist: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct UpdateCard {
    pub title: Option<String>,
    pub description: Option<String>,
    pub cover: Option<String>,
    pub start: Option<String>,
    pub due: Option<String>,
    pub due_complete: Option<bool>,
    pub reminder: Option<String>,
    pub recurrence: Option<String>,
    pub priority: Option<String>,
    pub is_milestone: Option<bool>,
    pub duration_days: Option<f64>,
    pub work_hours: Option<f64>,
    pub pct_complete: Option<i64>,
    pub task_type: Option<String>,
    pub units: Option<f64>,
    pub hours_per_day: Option<f64>,
    pub constraint_type: Option<String>,
    pub constraint_date: Option<String>,
    pub cost: Option<f64>,
    pub archived: Option<bool>,
}

/// Sections of a template copied when instantiating a card
/// (mirrors Trello's `keepFromSource`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CopySection {
    Checklists,
    Labels,
    Attachments,
    CustomFields,
    Cover,
}

impl CopySection {
    pub const ALL: [CopySection; 5] = [
        CopySection::Checklists,
        CopySection::Labels,
        CopySection::Attachments,
        CopySection::CustomFields,
        CopySection::Cover,
    ];
}

/// Toggle the template flag on a card (Trello "Make template").
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct SetTemplate {
    pub is_template: bool,
    pub template_description: Option<String>,
}

/// Instantiate a new card from a template: deep copy in one transaction.
/// Title and description are always copied; dates, completion status,
/// comments and activity never are.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct InstantiateTemplate {
    pub list_id: String,
    pub name: Option<String>,
    /// Sections to copy; None = all copyable sections.
    pub keep: Option<Vec<CopySection>>,
    /// "top" (default) | "bottom".
    pub at: Option<String>,
    /// Explicit floating position (wins over `at`).
    pub position: Option<f64>,
}

/// Move a card within its list or to another list.
/// Semantics:
/// - `position` provided → insert at that floating position.
/// - `before`/`after` card id → insert before/after that card.
/// - neither → append to end of target list.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct MoveCard {
    pub list_id: Option<String>,
    pub position: Option<f64>,
    pub before: Option<String>,
    pub after: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct CreateLabel {
    pub name: String,
    pub color: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct UpdateLabel {
    pub name: Option<String>,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct CreateChecklist {
    pub title: String,
    pub position: Option<f64>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct CreateChecklistItem {
    pub title: String,
    pub position: Option<f64>,
    pub due: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct UpdateChecklistItem {
    pub title: Option<String>,
    pub checked: Option<bool>,
    pub position: Option<f64>,
    pub due: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct CreateFieldDef {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub options: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct UpdateFieldDef {
    pub name: Option<String>,
    pub options: Option<String>,
    pub position: Option<f64>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct SetFieldValue {
    pub value: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct CreateDependency {
    pub predecessor_id: String,
    pub successor_id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub lag_days: Option<f64>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct UpdateDependency {
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub lag_days: Option<f64>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct CreateAutomation {
    pub name: String,
    pub trigger: String,
    pub conditions: Option<String>,
    pub actions: Option<String>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct UpdateAutomation {
    pub name: Option<String>,
    pub enabled: Option<bool>,
    pub trigger: Option<String>,
    pub conditions: Option<String>,
    pub actions: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct CreateCalendar {
    pub name: String,
    pub weekends: Option<String>,
    pub holidays: Option<String>,
    pub hours_per_day: Option<f64>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct UpdateCalendar {
    pub name: Option<String>,
    pub weekends: Option<String>,
    pub holidays: Option<String>,
    pub hours_per_day: Option<f64>,
}
