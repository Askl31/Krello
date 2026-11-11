mod common;

use common::{board_with_list, make_card, test_service};
use krello_core::models::*;

#[test]
fn delete_board_removes_it_and_undo_restores() {
    let s = test_service();
    let (b, l) = board_with_list(&s);
    make_card(&s, &l, "carte à perdre");
    s.delete_board(&b).unwrap();
    assert!(s.list_boards(true).unwrap().is_empty());
    assert!(s.get_board(&b).is_err());
    // undo restaure le board avec sa liste et sa carte
    s.undo().unwrap();
    let boards = s.list_boards(true).unwrap();
    assert_eq!(boards.len(), 1);
    assert_eq!(boards[0].id, b);
    let lists = s.list_lists(&b, true).unwrap();
    assert_eq!(lists.len(), 1);
    assert_eq!(s.list_cards(&b, true).unwrap().len(), 1);
}

#[test]
fn delete_all_boards_removes_everything_and_single_undo_restores_all() {
    let s = test_service();
    let (b1, l1) = board_with_list(&s);
    let b2 = s.create_board(CreateBoard { title: "Second".into(), ..Default::default() }).unwrap();
    let l2 = s.create_list(&b2.id, CreateList { title: "Inbox 2".into(), ..Default::default() }).unwrap();
    make_card(&s, &l1, "c1");
    make_card(&s, &l2.id, "c2");
    s.create_automation(
        &b1,
        CreateAutomation {
            name: "règle".into(),
            trigger: "card_created".into(),
            ..Default::default()
        },
    )
    .unwrap();
    // un board archivé est aussi supprimé
    s.update_board(&b2.id, UpdateBoard { archived: Some(true), ..Default::default() }).unwrap();

    let n = s.delete_all_boards().unwrap();
    assert_eq!(n, 2, "tous les boards, même archivés");
    assert!(s.list_boards(true).unwrap().is_empty());
    assert!(s.list_automations(&b1).unwrap().is_empty(), "règles cascade-supprimées");
    assert!(s.list_board_automation_runs(&b1, 10).unwrap().is_empty(), "journal nettoyé");

    // UNE seule entrée d'undo restaure les deux boards
    let label = s.undo().unwrap().unwrap();
    assert!(label.contains("2 boards"), "label: {label}");
    let boards = s.list_boards(true).unwrap();
    assert_eq!(boards.len(), 2);
    assert_eq!(s.list_cards(&b1, true).unwrap().len(), 1);
    assert_eq!(s.list_cards(&b2.id, true).unwrap().len(), 1);
}

#[test]
fn delete_board_unknown_is_not_found() {
    let s = test_service();
    let err = s.delete_board("board_inexistant").unwrap_err();
    assert!(matches!(err, krello_core::Error::NotFound(_)));
}
