mod common;

use common::{board_with_list, make_card, test_service};
use krello_core::models::*;

#[test]
fn card_create_defaults_and_validation() {
    let s = test_service();
    let (board_id, list_id) = board_with_list(&s);
    let c = s
        .create_card(
            &board_id,
            CreateCard {
                title: "Ma carte".into(),
                list_id: list_id.clone(),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(c.priority, "none");
    assert_eq!(c.position, 0.0);
    assert_eq!(c.board_id, board_id);

    // invalid priority rejected
    let err = s.create_card(
        &board_id,
        CreateCard {
            title: "x".into(),
            list_id: list_id.clone(),
            priority: Some("extrême".into()),
            ..Default::default()
        },
    );
    assert!(err.is_err());

    // empty title rejected
    let err = s.create_card(
        &board_id,
        CreateCard {
            title: "  ".into(),
            list_id,
            ..Default::default()
        },
    );
    assert!(err.is_err());
}

#[test]
fn card_positions_increment_and_move() {
    let s = test_service();
    let (_b, l1) = board_with_list(&s);
    let l2 = s
        .create_list(
            &s.get_list(&l1).unwrap().board_id,
            CreateList { title: "En cours".into(), ..Default::default() },
        )
        .unwrap()
        .id;

    let a = make_card(&s, &l1, "A");
    let b = make_card(&s, &l1, "B");
    let c = make_card(&s, &l1, "C");
    assert_eq!((a.position, b.position, c.position), (0.0, 1.0, 2.0));

    // Move C to top of l1 (nothing above A → A.pos - 1)
    let moved = s.move_card(&c.id, MoveCard { list_id: None, before: Some(a.id.clone()), after: None, position: None }).unwrap();
    assert_eq!(moved.position, -1.0);

    // Move B after C (cross-list to l2, appended)
    let moved2 = s.move_card(&b.id, MoveCard { list_id: Some(l2.clone()), ..Default::default() }).unwrap();
    assert_eq!((moved2.list_id.as_str(), moved2.position), (l2.as_str(), 0.0));

    // Reorder: A before B within l1 → renormalization not needed (gap fine)
    let _ = s.move_card(&a.id, MoveCard { before: Some(moved2.id.clone()), list_id: Some(l2.clone()), ..Default::default() }).unwrap();
}

#[test]
fn repeated_middle_inserts_renormalize_without_loss() {
    let s = test_service();
    let (b, l1) = board_with_list(&s);
    let mut ids = Vec::new();
    for i in 0..20 {
        ids.push(make_card(&s, &l1, &format!("carte-{i}")).id);
    }
    // repeatedly insert at the same gap → forces renormalization
    for _ in 0..50 {
        let first = s.list_cards(&b, false).unwrap();
        let first_in_list: Vec<_> = first.iter().filter(|c| c.list_id == l1).collect();
        let first_id = first_in_list[0].id.clone();
        s.move_card(&ids[7], MoveCard { before: Some(first_id), ..Default::default() }).unwrap();
    }
    let cards = s.list_cards(&b, false).unwrap();
    let mut in_list: Vec<_> = cards.iter().filter(|c| c.list_id == l1).map(|c| c.position).collect();
    in_list.sort_by(|a, b| a.partial_cmp(b).unwrap());
    // no duplicates
    for w in in_list.windows(2) {
        assert!((w[1] - w[0]).abs() > 1e-12, "duplicate positions detected");
    }
}

#[test]
fn update_card_due_complete_sets_completed_at() {
    let s = test_service();
    let (_b, l) = board_with_list(&s);
    let c = make_card(&s, &l, "Terminer");
    let updated = s
        .update_card(
            &c.id,
            UpdateCard {
                due_complete: Some(true),
                due: Some("2026-11-15T17:30:00".into()),
                priority: Some("high".into()),
                ..Default::default()
            },
        )
        .unwrap();
    assert!(updated.due_complete);
    assert!(updated.completed_at.is_some());
    let reverted = s
        .update_card(&c.id, UpdateCard { due_complete: Some(false), ..Default::default() })
        .unwrap();
    assert!(!reverted.due_complete);
    assert!(reverted.completed_at.is_none());
}

#[test]
fn undo_and_redo_card_update_move_delete() {
    let s = test_service();
    let (b, l1) = board_with_list(&s);
    let c = make_card(&s, &l1, "Originale");

    // update → create list → move
    s.update_card(&c.id, UpdateCard { title: Some("Modifiée".into()), ..Default::default() }).unwrap();
    let l2 = s.create_list(&b, CreateList { title: "Fait".into(), ..Default::default() }).unwrap().id;
    s.move_card(&c.id, MoveCard { list_id: Some(l2.clone()), ..Default::default() }).unwrap();

    // undo move → back in l1, title kept
    s.undo().unwrap();
    let got = s.get_card(&c.id).unwrap();
    assert_eq!(got.list_id, l1);
    assert_eq!(got.title, "Modifiée");

    // undo list creation, then undo update → title Originale
    s.undo().unwrap();
    s.undo().unwrap();
    let got = s.get_card(&c.id).unwrap();
    assert_eq!(got.title, "Originale");

    // redo update → list → move (redo pops in reverse order)
    s.redo().unwrap();
    let got = s.get_card(&c.id).unwrap();
    assert_eq!(got.title, "Modifiée");

    // children then delete then undo → full restore
    s.create_checklist(&c.id, CreateChecklist { title: "Checklist".into(), ..Default::default() }).unwrap();
    let cls = s.list_checklists(&c.id).unwrap();
    s.create_checklist_item(&cls[0].0.id, CreateChecklistItem { title: "item 1".into(), ..Default::default() }).unwrap();
    s.add_comment(&c.id, "un commentaire").unwrap();

    s.delete_card(&c.id).unwrap();
    assert!(s.get_card(&c.id).is_err());
    s.undo().unwrap();
    let got = s.get_card(&c.id).unwrap();
    assert_eq!(got.list_id, l1);
    let cls = s.list_checklists(&c.id).unwrap();
    assert_eq!(cls.len(), 1);
    assert_eq!(cls[0].1.len(), 1);
    assert_eq!(s.list_comments(&c.id).unwrap().len(), 1);
}

#[test]
fn cross_board_move_drops_labels() {
    let s = test_service();
    let (b1, l1) = board_with_list(&s);
    let b2 = s.create_board(CreateBoard { title: "Autre".into(), ..Default::default() }).unwrap();
    let l2 = s.create_list(&b2.id, CreateList { title: "Inbox".into(), ..Default::default() }).unwrap().id;

    let label = s.create_label(&b1, CreateLabel { name: "urgent".into(), color: "#ff0000".into() }).unwrap();
    let c = make_card(&s, &l1, "Transférer");
    s.attach_label(&c.id, &label.id).unwrap();

    s.move_card(&c.id, MoveCard { list_id: Some(l2.clone()), ..Default::default() }).unwrap();
    let moved = s.get_card(&c.id).unwrap();
    assert_eq!(moved.board_id, b2.id);

    let detail = s.card_detail(&c.id).unwrap();
    assert_eq!(detail["labels"].as_array().unwrap().len(), 0, "labels dropped on cross-board move");
}

#[test]
fn duplicate_card_copies_children() {
    let s = test_service();
    let (_b, l) = board_with_list(&s);
    let c = make_card(&s, &l, "Source");
    s.create_checklist(&c.id, CreateChecklist { title: "Étapes".into(), ..Default::default() }).unwrap();
    let cls = s.list_checklists(&c.id).unwrap();
    s.create_checklist_item(&cls[0].0.id, CreateChecklistItem { title: "x".into(), ..Default::default() }).unwrap();

    let dup = s.duplicate_card(&c.id).unwrap();
    assert!(dup.title.contains("(copie)"));
    assert_eq!(s.list_checklists(&dup.id).unwrap().len(), 1);
    assert_eq!(s.list_checklists(&dup.id).unwrap()[0].1.len(), 1);
}

#[test]
fn duplicate_board_copies_content() {
    let s = test_service();
    let (b, l) = board_with_list(&s);
    let c = make_card(&s, &l, "Tâche");
    let copy = s.duplicate_board(&b).unwrap();
    let lists = s.list_lists(&copy.id, false).unwrap();
    assert_eq!(lists.len(), 1);
    let cards = s.list_cards(&copy.id, false).unwrap();
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].title, "Tâche");
    let _ = c;
}

#[test]
fn labels_undo_attach_detach() {
    let s = test_service();
    let (b, l) = board_with_list(&s);
    let label = s.create_label(&b, CreateLabel { name: "bug".into(), color: "#f00".into() }).unwrap();
    let c = make_card(&s, &l, "Fix");
    s.attach_label(&c.id, &label.id).unwrap();
    assert_eq!(s.list_labels(&b).unwrap().len(), 1);
    s.undo().unwrap();
    let detail = s.card_detail(&c.id).unwrap();
    assert_eq!(detail["labels"].as_array().unwrap().len(), 0);
    s.redo().unwrap();
    let detail = s.card_detail(&c.id).unwrap();
    assert_eq!(detail["labels"].as_array().unwrap().len(), 1);
}

#[test]
fn custom_fields_validation_and_set() {
    let s = test_service();
    let (b, l) = board_with_list(&s);
    let def = s.create_field_def(&b, CreateFieldDef { name: "Estimation".into(), kind: "number".into(), options: None }).unwrap();
    let select_def = s
        .create_field_def(
            &b,
            CreateFieldDef {
                name: "Taille".into(),
                kind: "select".into(),
                options: Some(r#"["S","M","L"]"#.into()),
            },
        )
        .unwrap();
    let c = make_card(&s, &l, "Avec champs");

    s.set_field_value(&c.id, &def.id, "3.5").unwrap();
    s.set_field_value(&c.id, &select_def.id, "M").unwrap();
    // invalid select option
    assert!(s.set_field_value(&c.id, &select_def.id, "XXL").is_err());
    // invalid number
    assert!(s.set_field_value(&c.id, &def.id, "abc").is_err());

    let detail = s.card_detail(&c.id).unwrap();
    assert_eq!(detail["custom_fields"][&def.id], "3.5");

    // undo field set
    s.undo().unwrap(); // undo select "M"
    let detail = s.card_detail(&c.id).unwrap();
    assert!(detail["custom_fields"].get(&select_def.id).is_none());
}

#[test]
fn wip_limit_detection() {
    let s = test_service();
    let (b, l1) = board_with_list(&s);
    let l2 = s
        .create_list(
            &b,
            CreateList {
                title: "Limité".into(),
                wip_limit: Some(1),
                ..Default::default()
            },
        )
        .unwrap()
        .id;
    make_card(&s, &l2, "une");
    assert!(!s.wip_exceeded(&l2).unwrap(), "1/1 = at limit, not exceeded");
    make_card(&s, &l2, "deux");
    assert!(s.wip_exceeded(&l2).unwrap());
    assert!(!s.wip_exceeded(&l1).unwrap());
}
