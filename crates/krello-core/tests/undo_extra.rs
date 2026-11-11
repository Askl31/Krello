//! Undo coverage for calendars, baselines and attachments (previously not
//! undoable — the audit flagged these domains as missing UndoAction variants).

mod common;

use common::*;
use krello_core::models::*;

#[test]
fn calendar_crud_is_undoable() {
    let s = test_service();
    let cal = s
        .create_calendar(CreateCalendar { name: "Été".into(), ..Default::default() })
        .unwrap();
    assert!(s.list_calendars().unwrap().iter().any(|c| c.id == cal.id));

    // undo create
    assert_eq!(
        s.undo().unwrap().as_deref(),
        Some("Création du calendrier « Été »")
    );
    assert!(!s.list_calendars().unwrap().iter().any(|c| c.id == cal.id));
    // redo
    s.redo().unwrap();
    assert!(s.list_calendars().unwrap().iter().any(|c| c.id == cal.id));

    // update undoable
    let updated = s
        .update_calendar(&cal.id, UpdateCalendar { name: Some("Hiver".into()), ..Default::default() })
        .unwrap();
    assert_eq!(updated.name, "Hiver");
    s.undo().unwrap();
    assert_eq!(s.get_calendar(&cal.id).unwrap().name, "Été");

    // delete undoable
    s.delete_calendar(&cal.id).unwrap();
    assert!(s.get_calendar(&cal.id).is_err());
    s.undo().unwrap();
    assert_eq!(s.get_calendar(&cal.id).unwrap().name, "Été");
}

#[test]
fn baseline_create_and_delete_are_undoable_with_snapshots() {
    let s = test_service();
    let (board_id, list_id) = board_with_list(&s);
    let card = make_card(&s, &list_id, "tâche baseline");
    s.update_card(
        &card.id,
        UpdateCard { due: Some("2026-12-01".into()), ..Default::default() },
    )
    .unwrap();

    let b = s.create_baseline(&board_id, "v1").unwrap();
    assert_eq!(s.compare_baseline(&b.id).unwrap().len(), 1);

    // undo create
    s.undo().unwrap();
    assert!(s.list_baselines(&board_id).unwrap().is_empty());
    // redo
    s.redo().unwrap();
    assert_eq!(s.list_baselines(&board_id).unwrap().len(), 1);

    // delete → undo restaure baseline + snapshots comparables
    s.delete_baseline(&b.id).unwrap();
    assert!(s.list_baselines(&board_id).unwrap().is_empty());
    s.undo().unwrap();
    let cmp = s.compare_baseline(&b.id).unwrap();
    assert_eq!(cmp.len(), 1, "les snapshots doivent être restaurés par l'undo");
    assert_eq!(cmp[0].due_baseline.as_deref(), Some("2026-12-01"));
}

#[test]
fn attachment_add_and_delete_are_undoable() {
    let s = test_service();
    let (_board_id, list_id) = board_with_list(&s);
    let card = make_card(&s, &list_id, "pj");
    let att = s.add_attachment(&card.id, "a.txt", "text/plain", b"abc").unwrap();
    assert_eq!(s.list_attachments(&card.id).unwrap().len(), 1);

    // undo add → ligne supprimée + fichier purgé
    s.undo().unwrap();
    assert!(s.list_attachments(&card.id).unwrap().is_empty());
    assert!(
        s.attachment_path_checked(&att.stored_path).is_err(),
        "le fichier doit avoir été purgé après l'undo de l'ajout"
    );
    // redo add → ligne restaurée (le fichier était purgé : lecture 404 attendue)
    s.redo().unwrap();
    assert_eq!(s.list_attachments(&card.id).unwrap().len(), 1);

    // delete → undo restaure la ligne
    s.delete_attachment(&att.id).unwrap();
    assert!(s.list_attachments(&card.id).unwrap().is_empty());
    s.undo().unwrap();
    assert_eq!(s.list_attachments(&card.id).unwrap().len(), 1);
}
