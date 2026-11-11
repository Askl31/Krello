mod common;

use common::{board_with_list, make_card, test_service};
use krello_core::exim::{export_board, import_krello, import_trello, ExportFormat};
use krello_core::models::*;

fn trello_fixture() -> String {
    serde_json::json!({
        "name": "Board Trello",
        "lists": [
            { "id": "tl1", "name": "To Do", "pos": 1000, "closed": false },
            { "id": "tl2", "name": "Done", "pos": 2000, "closed": false }
        ],
        "labels": [
            { "id": "lab1", "name": "Rouge", "color": "red" }
        ],
        "cards": [
            {
                "id": "card1", "name": "Importer proprement", "desc": "une description",
                "idList": "tl1", "idLabels": ["lab1"], "pos": 1000,
                "due": "2026-12-01T10:00:00.000Z", "dueComplete": false, "closed": false,
                "customFieldItems": []
            },
            { "id": "card2", "name": "Déjà fait", "desc": "", "idList": "tl2", "idLabels": [], "pos": 1000, "dueComplete": true, "closed": false, "customFieldItems": [] }
        ],
        "checklists": [
            {
                "id": "chk1", "idCard": "card1", "name": "Sous-tâches",
                "checkItems": [
                    { "name": "item 1", "state": "complete", "pos": 1 },
                    { "name": "item 2", "state": "incomplete", "pos": 2 }
                ]
            }
        ],
        "customFields": [],
        "actions": [
            { "type": "commentCard", "date": "2026-11-01T10:00:00.000Z", "data": { "text": "un commentaire trello", "card": { "id": "card1" } } }
        ]
    })
    .to_string()
}

#[test]
fn json_export_roundtrip() {
    let s = test_service();
    let (b, l) = board_with_list(&s);
    let c = make_card(&s, &l, "Carte A");
    s.update_card(&c.id, UpdateCard {
        due: Some("2026-11-20T09:00:00".into()),
        priority: Some("high".into()),
        ..Default::default()
    }).unwrap();
    s.create_checklist(&c.id, CreateChecklist { title: "Étapes".into(), ..Default::default() }).unwrap();
    let cls = s.list_checklists(&c.id).unwrap();
    s.create_checklist_item(&cls[0].0.id, CreateChecklistItem { title: "première étape".into(), ..Default::default() }).unwrap();
    s.add_comment(&c.id, "note").unwrap();

    let json = s
        .with_db(|conn| krello_core::exim::export_json(conn, &b))
        .unwrap();

    let imported = import_krello(&s, &json).unwrap();
    let lists = s.list_lists(&imported.id, false).unwrap();
    assert_eq!(lists.len(), 1);
    let cards = s.list_cards(&imported.id, false).unwrap();
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].priority, "high");
    assert_eq!(cards[0].due.as_deref(), Some("2026-11-20T09:00:00"));
    let cls = s.list_checklists(&cards[0].id).unwrap();
    assert_eq!(cls.len(), 1);
    assert_eq!(cls[0].1.len(), 1);
    assert_eq!(s.list_comments(&cards[0].id).unwrap().len(), 1);
}

#[test]
fn trello_import_maps_lists_cards_labels_checklists_comments() {
    let s = test_service();
    let board = import_trello(&s, &trello_fixture()).unwrap();
    assert_eq!(board.title, "Board Trello");

    let lists = s.list_lists(&board.id, false).unwrap();
    assert_eq!(lists.len(), 2);
    assert_eq!(lists[0].title, "To Do");

    let cards = s.list_cards(&board.id, false).unwrap();
    assert_eq!(cards.len(), 2);
    let imported = cards.iter().find(|c| c.title == "Importer proprement").unwrap();
    assert_eq!(imported.description, "une description");
    assert!(imported.due.is_some());

    // label attached
    let detail = s.card_detail(&imported.id).unwrap();
    let labels = detail["labels"].as_array().unwrap();
    assert_eq!(labels.len(), 1);
    assert_eq!(labels[0]["name"], "Rouge");

    // checklist
    let cls = s.list_checklists(&imported.id).unwrap();
    assert_eq!(cls.len(), 1);
    assert_eq!(cls[0].1.len(), 2);
    assert!(cls[0].1[0].checked);
    assert!(!cls[0].1[1].checked);

    // comment
    assert_eq!(s.list_comments(&imported.id).unwrap().len(), 1);

    // dueComplete card marked complete
    let done = cards.iter().find(|c| c.title == "Déjà fait").unwrap();
    assert!(done.due_complete);
}

#[test]
fn markdown_csv_ics_exports() {
    let s = test_service();
    let (b, l) = board_with_list(&s);
    let c = make_card(&s, &l, "Livrer la démo");
    s.update_card(&c.id, UpdateCard {
        due: Some("2026-11-20".into()),
        description: Some("préparer l'environnement".into()),
        ..Default::default()
    }).unwrap();

    let (fname, mime, md) = s
        .with_db(|conn| export_board(conn, &b, ExportFormat::Markdown))
        .unwrap();
    assert!(fname.ends_with(".md"));
    assert_eq!(mime, "text/markdown");
    assert!(md.contains("# Projet Test"));
    assert!(md.contains("## À faire"));
    assert!(md.contains("- [ ] Livrer la démo"));
    assert!(md.contains("échéance: 2026-11-20"));
    assert!(md.contains("préparer l'environnement"));

    let (_, _, csv) = s
        .with_db(|conn| export_board(conn, &b, ExportFormat::Csv))
        .unwrap();
    assert!(csv.starts_with("titre;liste;"));
    assert!(csv.contains("Livrer la démo;À faire;"));

    let (_, _, ics) = s
        .with_db(|conn| export_board(conn, &b, ExportFormat::Ics))
        .unwrap();
    assert!(ics.contains("BEGIN:VCALENDAR"));
    assert!(ics.contains("UID:"));
    assert!(ics.contains("SUMMARY:Livrer la démo"));
    assert!(ics.contains("DTSTART;VALUE=DATE:20261120"));
}

#[test]
fn unknown_format_rejected() {
    assert!(ExportFormat::parse("xml").is_err());
    assert_eq!(ExportFormat::parse("md").unwrap(), ExportFormat::Markdown);
    assert_eq!(ExportFormat::parse("json").unwrap(), ExportFormat::Json);
    assert_eq!(ExportFormat::parse("ics").unwrap(), ExportFormat::Ics);
    assert_eq!(ExportFormat::parse("csv").unwrap(), ExportFormat::Csv);
}

#[test]
fn json_export_import_roundtrips_automations_with_remapped_ids() {
    let s = test_service();
    let (b, l) = board_with_list(&s);
    let l2 = s.create_list(&b, CreateList { title: "Terminé".into(), ..Default::default() }).unwrap().id;
    let label = s.create_label(&b, CreateLabel { name: "urgent".into(), color: "#f00".into() }).unwrap();
    s.create_automation(
        &b,
        CreateAutomation {
            name: "Regle test".into(),
            trigger: "card_moved".into(),
            conditions: Some(serde_json::json!({ "to_list_id": l2, "label_id": label.id }).to_string()),
            actions: Some(serde_json::json!([
                { "type": "move", "list_id": l, "position": "top" },
                { "type": "label", "label_id": label.id },
                { "type": "comment", "text": "Vu le {{date}}" }
            ]).to_string()),
            ..Default::default()
        },
    )
    .unwrap();

    let json = s.with_db(|conn| krello_core::exim::export_json(conn, &b)).unwrap();
    let imported = import_krello(&s, &json).unwrap();

    let autos = s.list_automations(&imported.id).unwrap();
    assert_eq!(autos.len(), 1, "la règle doit être réimportée");
    let a = &autos[0];
    assert_eq!(a.name, "Regle test");
    assert_eq!(a.trigger, "card_moved");
    // ids remappés vers les nouvelles listes/labels
    let lists_new = s.list_lists(&imported.id, false).unwrap();
    let labels_new = s.list_labels(&imported.id).unwrap();
    let l2_new = lists_new.iter().find(|x| x.title == "Terminé").unwrap().id.clone();
    let l_new = lists_new.iter().find(|x| x.title == "À faire").unwrap().id.clone();
    let label_new = labels_new.iter().find(|x| x.name == "urgent").unwrap().id.clone();
    let conds: serde_json::Value = serde_json::from_str(&a.conditions).unwrap();
    assert_eq!(conds["to_list_id"], l2_new);
    assert_eq!(conds["label_id"], label_new);
    let acts: Vec<serde_json::Value> = serde_json::from_str(&a.actions).unwrap();
    assert_eq!(acts[0]["list_id"], l_new);
    assert_eq!(acts[1]["label_id"], label_new);
    assert_eq!(acts[2]["text"], "Vu le {{date}}");

    // la règle importée fonctionne réellement sur le nouveau board
    let c = make_card(&s, &l_new, "carte roundtrip");
    s.attach_label(&c.id, &label_new).unwrap();
    s.move_card(&c.id, MoveCard { list_id: Some(l2_new), ..Default::default() }).unwrap();
    // action move -> la carte revient dans "À faire" (en haut) + label + commentaire
    let got = s.get_card(&c.id).unwrap();
    assert_eq!(got.list_id, l_new, "règle importée exécutée après remap");
    let comments = s.list_comments(&c.id).unwrap();
    assert!(comments.iter().any(|cm| cm.body.starts_with("Vu le")), "action comment de la règle importée");
}

#[test]
fn ics_export_contains_dtend_for_both_due_kinds() {
    let s = test_service();
    let (b, l) = board_with_list(&s);
    let c1 = make_card(&s, &l, "échéance date");
    s.update_card(&c1.id, UpdateCard {
        due: Some("2026-12-05".into()),
        ..Default::default()
    }).unwrap();
    let c2 = make_card(&s, &l, "échéance horodatée");
    s.update_card(&c2.id, UpdateCard {
        due: Some("2026-12-06T14:30:00".into()),
        ..Default::default()
    }).unwrap();

    let (_name, _mime, ics) = s
        .with_db(|conn| export_board(conn, &b, ExportFormat::parse("ics").unwrap()))
        .unwrap();
    assert!(ics.contains("DTSTART;VALUE=DATE:20261205"));
    assert!(ics.contains("DTEND;VALUE=DATE:20261206"), "date sans heure: DTEND exclusif = lendemain");
    assert!(ics.contains("DTSTART:20261206T143000"));
    assert!(ics.contains("DTEND:20261206T143000"), "échéance horodatée: DTEND explicite");
}
