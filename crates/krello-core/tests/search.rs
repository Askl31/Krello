mod common;

use common::{board_with_list, make_card, test_service};
use krello_core::models::*;
use krello_core::search::{search_cards, search_global, sanitize_fts, SearchFilters};

#[test]
fn search_matches_comment_bodies_when_enabled() {
    let s = test_service();
    let (_b, l) = board_with_list(&s);
    let hit = make_card(&s, &l, "Tâche A");
    make_card(&s, &l, "Tâche B");
    s.add_comment(&hit.id, "Contient le motunique dans un commentaire").unwrap();

    let with_comments = s
        .with_db(|conn| {
            search_cards(
                conn,
                Some("motunique"),
                &SearchFilters { include_comments: true, limit: 50, ..Default::default() },
            )
        })
        .unwrap();
    assert_eq!(with_comments.len(), 1, "le commentaire fait matcher la carte");
    assert_eq!(with_comments[0].id, hit.id);

    let without = s
        .with_db(|conn| {
            search_cards(conn, Some("motunique"), &SearchFilters { limit: 50, ..Default::default() })
        })
        .unwrap();
    assert!(without.is_empty(), "sans include_comments, les commentaires ne sont pas cherchés");
}

#[test]
fn global_search_includes_archived_cards_and_comments() {
    let s = test_service();
    let (_b, l) = board_with_list(&s);
    let archived = make_card(&s, &l, "Carte motdistinctif archivée");
    s.update_card(&archived.id, UpdateCard { archived: Some(true), ..Default::default() }).unwrap();
    let via_comment = make_card(&s, &l, "Ordinaire");
    s.add_comment(&via_comment.id, "voir motdistinctif dans les notes").unwrap();
    make_card(&s, &l, "Rien ici");

    let hits = s.with_db(|conn| search_global(conn, "motdistinctif", 50)).unwrap();
    let ids: Vec<&str> = hits.iter().map(|c| c.id.as_str()).collect();
    assert!(ids.contains(&archived.id.as_str()), "les cartes archivées sont cherchées globalement");
    assert!(ids.contains(&via_comment.id.as_str()), "les commentaires sont cherchés globalement");
}

#[test]
fn fts_sanitizer_quotes_terms_and_prefixes() {
    assert_eq!(sanitize_fts("devis client"), "\"devis\"* \"client\"*");
    assert_eq!(sanitize_fts("  "), "");
    assert_eq!(sanitize_fts("l'été \"interdit\""), "\"l'été\"* \"interdit\"*");
}

#[test]
fn full_text_search_matches_title_and_description() {
    let s = test_service();
    let (b, l) = board_with_list(&s);
    let c1 = make_card(&s, &l, "Facture courant");
    s.update_card(&c1.id, UpdateCard { description: Some("envoyer le devis au client".into()), ..Default::default() }).unwrap();
    make_card(&s, &l, "Autre chose");

    let hits = s
        .with_db(|conn| {
            search_cards(
                conn,
                Some("devis"),
                &SearchFilters { board_id: Some(b.clone()), ..Default::default() },
            )
        })
        .unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].id, c1.id);

    let hits = s
        .with_db(|conn| {
            search_cards(
                conn,
                Some("factur"), // prefix
                &SearchFilters { board_id: Some(b), ..Default::default() },
            )
        })
        .unwrap();
    assert_eq!(hits.len(), 1);
}

#[test]
fn filters_by_label_priority_and_due() {
    let s = test_service();
    let (b, l) = board_with_list(&s);
    let label = s.create_label(&b, CreateLabel { name: "urgent".into(), color: "#f00".into() }).unwrap();

    let c1 = make_card(&s, &l, "A");
    s.update_card(&c1.id, UpdateCard {
        priority: Some("urgent".into()),
        due: Some("2026-11-20".into()),
        ..Default::default()
    }).unwrap();
    let c2 = make_card(&s, &l, "B");
    s.attach_label(&c2.id, &label.id).unwrap();
    make_card(&s, &l, "C");

    let by_priority = s
        .with_db(|conn| {
            search_cards(conn, None, &SearchFilters {
                board_id: Some(b.clone()),
                priority: Some("urgent".into()),
                ..Default::default()
            })
        })
        .unwrap();
    assert_eq!(by_priority.len(), 1);
    assert_eq!(by_priority[0].id, c1.id);

    let by_label = s
        .with_db(|conn| {
            search_cards(conn, None, &SearchFilters {
                board_id: Some(b.clone()),
                label_id: Some(label.id.clone()),
                ..Default::default()
            })
        })
        .unwrap();
    assert_eq!(by_label.len(), 1);
    assert_eq!(by_label[0].id, c2.id);

    let due_before = s
        .with_db(|conn| {
            search_cards(conn, None, &SearchFilters {
                board_id: Some(b.clone()),
                due_before: Some("2026-11-25".into()),
                has_due: Some(true),
                ..Default::default()
            })
        })
        .unwrap();
    assert_eq!(due_before.len(), 1);

    let combined = s
        .with_db(|conn| {
            search_cards(conn, Some("b"), &SearchFilters {
                board_id: Some(b),
                label_id: Some(label.id),
                ..Default::default()
            })
        })
        .unwrap();
    assert_eq!(combined.len(), 1, "FTS 'b*' + label filter → card B");
}
