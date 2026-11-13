//! Déplacement contrôlé des listes : swap avec le voisin immédiat, bords
//! no-op, listes archivées exclues du voisinage, undo en une entrée.

mod common;

use common::test_service;
use krello_core::models::*;
use krello_core::service::Service;

/// Board + listes créées dans l'ordre ; retourne (board_id, [list_id…]).
fn board_with_lists(s: &Service, titles: &[&str]) -> (String, Vec<String>) {
    let b = s.create_board(CreateBoard { title: "Board".into(), ..Default::default() }).unwrap();
    let ids = titles
        .iter()
        .map(|t| s.create_list(&b.id, CreateList { title: (*t).into(), ..Default::default() }).unwrap().id)
        .collect();
    (b.id, ids)
}

/// Titres des listes ACTIVES dans l'ordre d'affichage (positions serveur).
fn display_order(s: &Service, board_id: &str) -> Vec<String> {
    s.list_lists(board_id, false)
        .unwrap()
        .into_iter()
        .map(|l| l.title)
        .collect()
}

#[test]
fn move_list_swaps_with_immediate_neighbor() {
    let s = test_service();
    let (board_id, ids) = board_with_lists(&s, &["L1", "L2", "L3"]);

    // L2 vers la gauche → L2, L1, L3
    let moved = s.move_list(&ids[1], -1).unwrap();
    assert_eq!(moved.title, "L2");
    assert_eq!(display_order(&s, &board_id), vec!["L2", "L1", "L3"]);
    let positions: Vec<f64> = ids.iter().map(|id| s.get_list(id).unwrap().position).collect();
    assert_eq!(positions, vec![1.0, 0.0, 2.0], "positions échangées, L3 intacte");

    // Undo → ordre d'origine ET positions restaurées.
    s.undo().unwrap();
    assert_eq!(display_order(&s, &board_id), vec!["L1", "L2", "L3"]);
    let positions: Vec<f64> = ids.iter().map(|id| s.get_list(id).unwrap().position).collect();
    assert_eq!(positions, vec![0.0, 1.0, 2.0]);
}

#[test]
fn move_list_at_edges_is_noop() {
    let s = test_service();
    let (board_id, ids) = board_with_lists(&s, &["L1", "L2"]);

    // L1 vers la gauche → no-op (déjà en tête).
    let same = s.move_list(&ids[0], -1).unwrap();
    assert_eq!(same.title, "L1");
    assert_eq!(display_order(&s, &board_id), vec!["L1", "L2"]);
    assert_eq!(s.get_list(&ids[0]).unwrap().position, 0.0);

    // L2 vers la droite → no-op (déjà en queue).
    let same = s.move_list(&ids[1], 1).unwrap();
    assert_eq!(same.title, "L2");
    assert_eq!(display_order(&s, &board_id), vec!["L1", "L2"]);

    // Direction invalide → 400.
    let err = s.move_list(&ids[0], 5);
    assert!(matches!(err.unwrap_err(), krello_core::Error::Validation(_)));
    let err = s.move_list(&ids[0], 0);
    assert!(matches!(err.unwrap_err(), krello_core::Error::Validation(_)));
}

#[test]
fn move_list_skips_archived_neighbors() {
    let s = test_service();
    let (board_id, ids) = board_with_lists(&s, &["L1", "L2", "L3"]);
    // Archiver L1 : L2 devient la première active → vers la gauche = no-op.
    s.update_list(&ids[0], UpdateList { archived: Some(true), ..Default::default() }).unwrap();
    let same = s.move_list(&ids[1], -1).unwrap();
    assert_eq!(same.title, "L2");
    assert_eq!(display_order(&s, &board_id), vec!["L2", "L3"], "l'archivée n'apparaît plus");

    // L3 vers la gauche → swap avec L2 (première active au-dessus).
    let moved = s.move_list(&ids[2], -1).unwrap();
    assert_eq!(moved.title, "L3");
    assert_eq!(display_order(&s, &board_id), vec!["L3", "L2"]);
}

#[test]
fn move_list_unknown_is_not_found() {
    let s = test_service();
    let err = s.move_list("list_inexistante", 1);
    assert!(matches!(err.unwrap_err(), krello_core::Error::NotFound(_)));
}
