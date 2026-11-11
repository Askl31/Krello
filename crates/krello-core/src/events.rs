//! Change events broadcast to WebSocket clients (and used to trigger UI refresh).

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "data")]
pub enum Event {
    BoardCreated { board_id: String },
    BoardUpdated { board_id: String },
    BoardDeleted { board_id: String },
    ListCreated { board_id: String, list_id: String },
    ListUpdated { board_id: String, list_id: String },
    ListDeleted { board_id: String, list_id: String },
    CardCreated { board_id: String, card_id: String },
    CardUpdated { board_id: String, card_id: String },
    CardMoved { board_id: String, card_id: String },
    CardDeleted { board_id: String, card_id: String },
    LabelsUpdated { board_id: String, card_id: Option<String> },
    ChecklistsUpdated { board_id: String, card_id: String },
    CommentsUpdated { board_id: String, card_id: String },
    AttachmentsUpdated { board_id: String, card_id: String },
    FieldsUpdated { board_id: String, card_id: Option<String> },
    DependenciesUpdated { board_id: String },
    ScheduleUpdated { board_id: String },
    /// Reminder fired by the sweep for a card whose reminder moment is reached.
    /// `title`/`due` travel along so clients can toast without refetching.
    ReminderDue { board_id: String, card_id: String, title: String, due: String },
    AutomationsUpdated { board_id: String },
    AutomationRan { board_id: String },
    BaselinesUpdated { board_id: String },
    CalendarsUpdated { calendar_id: String },
    UndoApplied { board_id: Option<String> },
}
