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
fn pinned_boards_sort_first_and_undo_restores() {
    let s = test_service();
    let b1 = s.create_board(CreateBoard { title: "Un".into(), ..Default::default() }).unwrap().id;
    let b2 = s.create_board(CreateBoard { title: "Deux".into(), ..Default::default() }).unwrap().id;
    let b3 = s.create_board(CreateBoard { title: "Trois".into(), ..Default::default() }).unwrap().id;
    // Épingler Trois puis Un : le dernier épinglé passe en tête.
    s.update_board(&b3, UpdateBoard { pinned: Some(true), ..Default::default() }).unwrap();
    s.update_board(&b1, UpdateBoard { pinned: Some(true), ..Default::default() }).unwrap();
    let boards = s.list_boards(false).unwrap();
    let ids: Vec<&str> = boards.iter().map(|b| b.id.as_str()).collect();
    assert_eq!(ids, vec![b1.as_str(), b3.as_str(), b2.as_str()]);
    assert!(boards[0].pinned && boards[1].pinned && !boards[2].pinned);

    // Undo du dernier pin → Un redevient non épinglé, l'ordre suit.
    s.undo().unwrap();
    let b1_board = s.get_board(&b1).unwrap();
    assert!(!b1_board.pinned);
    assert!(b1_board.pinned_at.is_none());
    let boards = s.list_boards(false).unwrap();
    let ids: Vec<&str> = boards.iter().map(|b| b.id.as_str()).collect();
    assert_eq!(ids, vec![b3.as_str(), b1.as_str(), b2.as_str()]);
}

#[test]
fn board_templates_gallery_lifecycle() {
    let s = test_service();
    let (b, l) = board_with_list(&s);
    make_card(&s, &l, "carte modèle");

    // Pas encore un modèle : l'instanciation échoue.
    assert!(s.instantiate_board_template(&b, None).is_err());

    s.set_board_template(
        &b,
        SetBoardTemplate { is_template: true, template_description: Some("Process de lancement".into()) },
    )
    .unwrap();

    // Exclu de la liste normale, présent dans la galerie.
    assert!(s.list_boards(false).unwrap().iter().all(|x| x.id != b), "le modèle est masqué de la liste");
    let gallery = s.list_board_templates().unwrap();
    assert_eq!(gallery.len(), 1);
    assert_eq!(gallery[0].template_description.as_deref(), Some("Process de lancement"));

    // Instanciation = copie profonde, non-modèle, titre du modèle par défaut.
    let inst = s.instantiate_board_template(&b, None).unwrap();
    assert_eq!(inst.title, "Projet Test");
    assert!(!inst.is_board_template);
    assert_eq!(s.list_boards(false).unwrap().len(), 1, "seule l'instanciation est visible");
    assert_eq!(s.list_lists(&inst.id, false).unwrap().len(), 1);
    let cards = s.list_cards(&inst.id, false).unwrap();
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].title, "carte modèle");
    assert_ne!(cards[0].id, s.list_cards(&b, false).unwrap()[0].id, "ids régénérés");

    // Titre explicite : prioritaire sur le titre du modèle.
    let named = s.instantiate_board_template(&b, Some("  Sprint 42  ".into())).unwrap();
    assert_eq!(named.title, "Sprint 42");

    // Undo de la dernière instanciation supprime la copie.
    s.undo().unwrap();
    assert!(s.get_board(&named.id).is_err());

    // Déconvertir le modèle le rend de nouveau visible.
    s.set_board_template(&b, SetBoardTemplate { is_template: false, template_description: None }).unwrap();
    let visible = s.list_boards(false).unwrap();
    assert_eq!(visible.len(), 2, "modèle + instanciation restante");
    assert!(visible[0].template_description.is_none(), "description purgée à la déconversion");
}

#[test]
fn global_stats_counts_across_boards() {
    let s = test_service();
    let (b1, l1) = board_with_list(&s);
    let b2 = s.create_board(CreateBoard { title: "Second".into(), ..Default::default() }).unwrap();
    let l2 = s.create_list(&b2.id, CreateList { title: "L2".into(), ..Default::default() }).unwrap();

    let today = krello_core::time::today();
    let yesterday = krello_core::time::add_days(today, -1).to_string();
    let plus3 = krello_core::time::add_days(today, 3).to_string();
    let in8 = krello_core::time::add_days(today, 8).to_string();

    let late = make_card(&s, &l1, "en retard");
    s.update_card(&late.id, UpdateCard { due: Some(yesterday), ..Default::default() }).unwrap();
    let soon = make_card(&s, &l1, "à venir");
    s.update_card(&soon.id, UpdateCard { due: Some(plus3), ..Default::default() }).unwrap();
    let far = make_card(&s, &l1, "lointaine");
    s.update_card(&far.id, UpdateCard { due: Some(in8), ..Default::default() }).unwrap();
    let done = make_card(&s, &l2.id, "terminée");
    s.update_card(&done.id, UpdateCard { due_complete: Some(true), ..Default::default() }).unwrap();
    make_card(&s, &l2.id, "sans date");
    s.add_comment(&done.id, "fait aujourd'hui").unwrap();

    let st = s.global_stats().unwrap();
    assert_eq!(st["boards"], 2);
    assert_eq!(st["cards_active"], 5);
    assert_eq!(st["overdue"], 1);
    assert_eq!(st["due_7d"], 1, "lointaine (8 j) exclue");
    assert_eq!(st["done_7d"], 1);
    let per_board = st["per_board"].as_array().unwrap();
    assert_eq!(per_board.len(), 2);
    assert_eq!(per_board[0]["title"], "Projet Test", "premier board créé en tête");
    assert_eq!(per_board[0]["active"], 3);
    assert_eq!(per_board[0]["overdue"], 1);
    assert!(st["activity"].as_array().unwrap().len() >= 1, "activité récente présente");
}

#[test]
fn global_stats_urgent_and_due_soon_lists() {
    let s = test_service();
    let (_b1, l1) = board_with_list(&s);
    let b2 = s.create_board(CreateBoard { title: "Second".into(), ..Default::default() }).unwrap();
    let l2 = s.create_list(&b2.id, CreateList { title: "L2".into(), ..Default::default() }).unwrap();

    let today = krello_core::time::today();
    let yesterday = krello_core::time::add_days(today, -1).to_string();
    let today_s = today.to_string();
    let tomorrow = krello_core::time::add_days(today, 1).to_string();
    let in2 = krello_core::time::add_days(today, 2).to_string();
    let in3 = krello_core::time::add_days(today, 3).to_string();

    let u1 = make_card(&s, &l1, "urgente 1");
    s.update_card(&u1.id, UpdateCard { priority: Some("urgent".into()), ..Default::default() }).unwrap();
    let h1 = make_card(&s, &l1, "haute 1");
    s.update_card(&h1.id, UpdateCard { priority: Some("high".into()), due: Some(yesterday.clone()), ..Default::default() }).unwrap();
    make_card(&s, &l1, "moyenne"); // priorité medium → exclue d'« urgentes »

    let u_done = make_card(&s, &l2.id, "urgente terminée");
    s.update_card(&u_done.id, UpdateCard {
        priority: Some("urgent".into()),
        due_complete: Some(true),
        ..Default::default()
    }).unwrap();

    let d_today = make_card(&s, &l2.id, "échéance aujourd'hui");
    s.update_card(&d_today.id, UpdateCard { due: Some(today_s.clone()), ..Default::default() }).unwrap();
    let d_tomorrow = make_card(&s, &l2.id, "échéance demain");
    s.update_card(&d_tomorrow.id, UpdateCard { due: Some(tomorrow.clone()), ..Default::default() }).unwrap();
    let d_in2 = make_card(&s, &l2.id, "échéance +2");
    s.update_card(&d_in2.id, UpdateCard { due: Some(in2.clone()), ..Default::default() }).unwrap();
    let d_in3 = make_card(&s, &l2.id, "échéance +3");
    s.update_card(&d_in3.id, UpdateCard { due: Some(in3.clone()), ..Default::default() }).unwrap();
    let d_late = make_card(&s, &l2.id, "en retard");
    s.update_card(&d_late.id, UpdateCard { due: Some(yesterday.clone()), due_complete: Some(true), ..Default::default() }).unwrap();

    // Board archivé : ses cartes urgentes n'apparaissent plus.
    let b3 = s.create_board(CreateBoard { title: "Archivé".into(), ..Default::default() }).unwrap();
    let l3 = s.create_list(&b3.id, CreateList { title: "L3".into(), ..Default::default() }).unwrap().id;
    let u_arch = make_card(&s, &l3, "urgente board archivé");
    s.update_card(&u_arch.id, UpdateCard { priority: Some("urgent".into()), ..Default::default() }).unwrap();
    s.update_board(&b3.id, UpdateBoard { archived: Some(true), ..Default::default() }).unwrap();

    let st = s.global_stats().unwrap();

    // Urgentes : urgent d'abord, puis high ; terminées/archivées/boards archivés exclus.
    let urgent = st["urgent"].as_array().unwrap();
    let urgent_titles: Vec<&str> = urgent.iter().map(|c| c["title"].as_str().unwrap()).collect();
    assert_eq!(urgent_titles, vec!["urgente 1", "haute 1"]);
    assert_eq!(st["urgent_total"], 2);

    // Échéances proches : aujourd'hui → +2 inclus, hier et +3 exclus, tri par due.
    let due_soon = st["due_soon"].as_array().unwrap();
    let due_titles: Vec<&str> = due_soon.iter().map(|c| c["title"].as_str().unwrap()).collect();
    assert_eq!(due_titles, vec!["échéance aujourd'hui", "échéance demain", "échéance +2"]);
    assert_eq!(st["due_soon_total"], 3);
}

#[test]
fn delete_board_unknown_is_not_found() {
    let s = test_service();
    let err = s.delete_board("board_inexistant").unwrap_err();
    assert!(matches!(err, krello_core::Error::NotFound(_)));
}
