//! The public recompute endpoint must run the CPM exactly once and emit a
//! single ScheduleUpdated event (regression: it used to recompute twice).

mod common;

use common::*;
use krello_core::events::Event;

#[test]
fn recompute_public_emits_exactly_one_schedule_event() {
    let s = test_service();
    let (board_id, list_id) = board_with_list(&s);
    make_card(&s, &list_id, "tâche A");
    let mut rx = s.subscribe();
    let summary = s.recompute_and_notify_public(&board_id).unwrap();
    assert_eq!(summary.board_id, board_id);
    let mut count = 0;
    while let Ok(ev) = rx.try_recv() {
        if matches!(ev.event, Event::ScheduleUpdated { .. }) {
            count += 1;
        }
    }
    assert_eq!(count, 1, "exactement un événement ScheduleUpdated attendu");
}
