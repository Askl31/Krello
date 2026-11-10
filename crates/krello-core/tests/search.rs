mod common;

use common::{board_with_list, make_card, test_service};
use krello_core::models::*;
use krello_core::search::{search_cards, sanitize_fts, SearchFilters};

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
