//! Tests du deep link krello:// (ajout rapide Siri/Spotlight).

mod common;

use krello_core::deeplink::{parse_deep_link, percent_decode, DeepLinkOutcome};
use krello_core::models::*;
use krello_core::service::Service;

fn svc() -> (tempfile::TempDir, Service) {
    let tmp = tempfile::tempdir().unwrap();
    let db = krello_core::db::Db::open_memory().unwrap();
    let s = Service::new(db, tmp.path().join("attachments"));
    (tmp, s)
}

fn board_with_lists(s: &Service, lists: &[&str]) -> (String, Vec<String>) {
    let b = s
        .create_board(CreateBoard {
            title: "Projet".into(),
            ..Default::default()
        })
        .unwrap();
    let ids = lists
        .iter()
        .map(|t| s.create_list(&b.id, CreateList { title: t.to_string(), ..Default::default() }).unwrap().id)
        .collect();
    (b.id, ids)
}

#[test]
fn percent_decode_works() {
    assert_eq!(percent_decode("Acheter%20du%20lait"), "Acheter du lait");
    assert_eq!(percent_decode("caf%C3%A9"), "café");
    assert_eq!(percent_decode("a+b"), "a b");
    assert_eq!(percent_decode("100%25"), "100%");
    assert_eq!(percent_decode("%zz"), "%zz"); // hex invalide conservé
}

#[test]
fn parse_full_link() {
    let l = parse_deep_link("krello://task/new?list=Courses&board=Perso&title=Acheter%20du%20lait&confirm=1").unwrap();
    assert_eq!(l.action, "task/new");
    assert_eq!(l.list.as_deref(), Some("Courses"));
    assert_eq!(l.board.as_deref(), Some("Perso"));
    assert_eq!(l.title.as_deref(), Some("Acheter du lait"));
    assert!(l.confirm);
}

#[test]
fn parse_rejects_bad_scheme_and_missing_action() {
    assert!(parse_deep_link("https://task/new").is_err());
    assert!(parse_deep_link("krello://").is_err());
    let l = parse_deep_link("krello://task/add").unwrap();
    assert_eq!(l.action, "task/add");
    assert!(!l.confirm);
    assert!(l.list.is_none());
}

#[test]
fn handle_creates_card_in_named_list() {
    let (_tmp, s) = svc();
    let (_b, lists) = board_with_lists(&s, &["À faire", "Courses"]);
    let out = s
        .handle_deep_link("krello://task/new?list=courses&title=Acheter%20du%20lait")
        .unwrap();
    let DeepLinkOutcome::Created { card, list, .. } = out else {
        panic!("attendu Created")
    };
    assert_eq!(card.title, "Acheter du lait");
    assert_eq!(card.list_id, lists[1]); // résolution par nom insensible à la casse
    assert_eq!(list.title, "Courses");
}

#[test]
fn handle_default_list_when_no_param() {
    let (_tmp, s) = svc();
    let (_b, lists) = board_with_lists(&s, &["Backlog"]);
    let out = s.handle_deep_link("krello://task/new?title=Test").unwrap();
    let DeepLinkOutcome::Created { card, .. } = out else { panic!("attendu Created") };
    assert_eq!(card.list_id, lists[0]);
}

#[test]
fn handle_needs_input_when_no_title() {
    let (_tmp, s) = svc();
    board_with_lists(&s, &["À faire"]);
    let out = s.handle_deep_link("krello://task/new?list=À%20faire").unwrap();
    let DeepLinkOutcome::NeedsInput { title, list, .. } = out else {
        panic!("attendu NeedsInput")
    };
    assert_eq!(title, "");
    assert_eq!(list.title, "À faire");
}

#[test]
fn handle_confirm_flag_returns_needs_input_with_title() {
    let (_tmp, s) = svc();
    board_with_lists(&s, &["À faire"]);
    let out = s
        .handle_deep_link("krello://task/new?list=À%20faire&title=Test&confirm=1")
        .unwrap();
    let DeepLinkOutcome::NeedsInput { title, .. } = out else {
        panic!("attendu NeedsInput")
    };
    assert_eq!(title, "Test");
}

#[test]
fn handle_reads_and_deletes_siri_inbox() {
    let (tmp, s) = svc();
    board_with_lists(&s, &["Courses"]);
    let inbox = s.siri_inbox_path();
    assert!(inbox.starts_with(tmp.path()));
    std::fs::create_dir_all(inbox.parent().unwrap()).unwrap();
    std::fs::write(&inbox, "Appeler le plombier\n").unwrap();
    let out = s.handle_deep_link("krello://task/new?list=Courses").unwrap();
    let DeepLinkOutcome::Created { card, .. } = out else {
        panic!("attendu Created")
    };
    assert_eq!(card.title, "Appeler le plombier");
    assert!(!inbox.exists(), "l'inbox doit être supprimée après consommation");
}

#[test]
fn handle_unknown_list_is_not_found() {
    let (_tmp, s) = svc();
    board_with_lists(&s, &["À faire"]);
    let err = s.handle_deep_link("krello://task/new?list=Nulle%20part&title=X").unwrap_err();
    assert!(format!("{err}").contains("introuvable"));
}

#[test]
fn handle_rejects_unknown_action() {
    let (_tmp, s) = svc();
    let err = s.handle_deep_link("krello://board/open?id=x").unwrap_err();
    assert!(format!("{err}").contains("task/new"));
}

#[test]
fn find_list_prefers_id_then_name_and_board_filter() {
    let (_tmp, s) = svc();
    let (b1, l1) = board_with_lists(&s, &["Courses"]);
    let b2 = s
        .create_board(CreateBoard { title: "Travail".into(), ..Default::default() })
        .unwrap();
    let l2 = s.create_list(&b2.id, CreateList { title: "Courses".into(), ..Default::default() }).unwrap();
    // par id exact
    let (l, b) = s.find_list(&l1[0], None).unwrap();
    assert_eq!(l.id, l1[0]);
    assert_eq!(b.id, b1);
    // par nom sans board → premier board par created_at (b1)
    let (l, b) = s.find_list("courses", None).unwrap();
    assert_eq!(l.id, l1[0]);
    assert_eq!(b.id, b1);
    // par nom + board
    let (l, b) = s.find_list("Courses", Some("Travail")).unwrap();
    assert_eq!(l.id, l2.id);
    assert_eq!(b.id, b2.id);
}
