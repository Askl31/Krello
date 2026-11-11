//! Effective recurrence: completing a recurring card spawns the next
//! occurrence (same attributes, due advanced one step), undoable as one
//! operation; `until` ends the series; malformed recurrence never blocks
//! completion.

mod common;

use common::*;
use krello_core::models::*;
use krello_core::service::Service;

fn recurring_card(s: &Service, list_id: &str, due: &str, recurrence: &str) -> Card {
    let c = make_card(s, list_id, "tâche récurrente");
    s.update_card(
        &c.id,
        UpdateCard {
            due: Some(due.into()),
            recurrence: Some(recurrence.into()),
            ..Default::default()
        },
    )
    .unwrap();
    c
}

#[test]
fn completing_daily_card_creates_next_occurrence() {
    let s = test_service();
    let (_board_id, list_id) = board_with_list(&s);
    let c = recurring_card(&s, &list_id, "2026-11-10", r#"{"freq":"daily","interval":2}"#);

    s.update_card(&c.id, UpdateCard { due_complete: Some(true), ..Default::default() }).unwrap();

    let cards = s.list_cards(&_board_id, false).unwrap();
    assert_eq!(cards.len(), 2, "l'occurrence suivante doit exister");
    let next = cards.iter().find(|x| x.id != c.id).unwrap();
    assert_eq!(next.due.as_deref(), Some("2026-11-12"), "+2 jours");
    assert!(!next.due_complete);
    assert_eq!(next.title, c.title);
    assert_eq!(next.completed_at, None);
    assert_eq!(next.pct_complete, 0, "nouvelle occurrence non commencée");
}

#[test]
fn weekly_and_monthly_steps_and_timed_due() {
    let s = test_service();
    let (_board_id, list_id) = board_with_list(&s);

    let w = recurring_card(&s, &list_id, "2026-11-02", r#"{"freq":"weekly"}"#);
    s.update_card(&w.id, UpdateCard { due_complete: Some(true), ..Default::default() }).unwrap();
    let cards = s.list_cards(&_board_id, false).unwrap();
    let w_next = cards
        .iter()
        .find(|x| x.due.as_deref() == Some("2026-11-09"))
        .expect("weekly = +7 jours");
    assert_eq!(w_next.due.as_deref(), Some("2026-11-09"));

    let m = recurring_card(&s, &list_id, "2026-01-31", r#"{"freq":"monthly","interval":1}"#);
    s.update_card(&m.id, UpdateCard { due_complete: Some(true), ..Default::default() }).unwrap();
    let cards = s.list_cards(&_board_id, false).unwrap();
    let m_next = cards
        .iter()
        .find(|x| x.due.as_deref() == Some("2026-02-28"))
        .expect("fin de mois clampée par chrono");
    assert_eq!(m_next.due.as_deref(), Some("2026-02-28"));

    let t = recurring_card(&s, &list_id, "2026-03-10T09:30:00", r#"{"freq":"daily"}"#);
    s.update_card(&t.id, UpdateCard { due_complete: Some(true), ..Default::default() }).unwrap();
    let cards = s.list_cards(&_board_id, false).unwrap();
    let t_next = cards
        .iter()
        .find(|x| x.due.as_deref() == Some("2026-03-11T09:30:00"))
        .expect("l'heure de l'échéance doit être conservée");
    assert_eq!(t_next.id, t_next.id);
}

#[test]
fn until_ends_the_series_and_malformed_does_not_block() {
    let s = test_service();
    let (_board_id, list_id) = board_with_list(&s);

    // until atteint → pas de successeur.
    let c = recurring_card(&s, &list_id, "2026-11-10", r#"{"freq":"daily","until":"2026-11-10"}"#);
    s.update_card(&c.id, UpdateCard { due_complete: Some(true), ..Default::default() }).unwrap();
    assert_eq!(s.list_cards(&_board_id, false).unwrap().len(), 1, "série terminée");

    // Récurrence malformée → complétion réussie quand même, pas de successeur.
    let c2 = make_card(&s, &list_id, "récurrence cassée");
    s.update_card(&c2.id, UpdateCard {
        due: Some("2026-11-10".into()),
        recurrence: Some(r#"{"freq":"pluton"}"#.into()),
        ..Default::default()
    }).unwrap();
    let done = s.update_card(&c2.id, UpdateCard { due_complete: Some(true), ..Default::default() }).unwrap();
    assert!(done.due_complete, "la complétion ne doit pas être bloquée");
    assert_eq!(s.list_cards(&_board_id, false).unwrap().len(), 2, "les 2 cartes complétées, aucun successeur");
}

#[test]
fn recurrence_successor_is_undoable_as_one_operation() {
    let s = test_service();
    let (board_id, list_id) = board_with_list(&s);
    let c = recurring_card(&s, &list_id, "2026-11-10", r#"{"freq":"daily"}"#);

    s.update_card(&c.id, UpdateCard { due_complete: Some(true), ..Default::default() }).unwrap();
    assert_eq!(s.list_cards(&board_id, false).unwrap().len(), 2);

    // Un seul ⌘Z annule l'update ET l'occurrence suivante.
    s.undo().unwrap();
    let cards = s.list_cards(&board_id, false).unwrap();
    assert_eq!(cards.len(), 1, "l'undo retire le successeur");
    let undone = cards.iter().find(|x| x.id == c.id).unwrap();
    assert!(!undone.due_complete, "la complétion est annulée aussi");

    // Redo restaure les deux.
    s.redo().unwrap();
    assert_eq!(s.list_cards(&board_id, false).unwrap().len(), 2);
}
