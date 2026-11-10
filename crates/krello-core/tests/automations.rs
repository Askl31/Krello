mod common;

use common::{board_with_list, make_card, test_service};
use krello_core::models::*;

#[test]
fn card_created_trigger_moves_new_cards() {
    let s = test_service();
    let (b, inbox) = board_with_list(&s);
    let todo = s.create_list(&b, CreateList { title: "À faire".into(), ..Default::default() }).unwrap();

    // Rule: every card created in Inbox gets the "urgent" label and high priority.
    let label = s.create_label(&b, CreateLabel { name: "urgent".into(), color: "#f00".into() }).unwrap();
    s.create_automation(
        &b,
        CreateAutomation {
            name: "Urgent Inbox".into(),
            trigger: "card_created".into(),
            conditions: Some(serde_json::json!({ "list_id": inbox }).to_string()),
            actions: Some(serde_json::json!([
                { "type": "label", "label_id": label.id },
                { "type": "priority", "value": "high" }
            ]).to_string()),
            ..Default::default()
        },
    )
    .unwrap();

    let c = make_card(&s, &inbox, "Nouvelle demande");
    let updated = s.get_card(&c.id).unwrap();
    assert_eq!(updated.priority, "high", "automation must have set priority");
    let detail = s.card_detail(&c.id).unwrap();
    let labels = detail["labels"].as_array().unwrap();
    assert_eq!(labels.len(), 1);
    assert_eq!(labels[0]["name"], "urgent");
}

#[test]
fn move_trigger_archives_cards_in_done() {
    let s = test_service();
    let (b, todo) = board_with_list(&s);
    let done = s.create_list(&b, CreateList { title: "Terminé".into(), ..Default::default() }).unwrap().id;
    s.create_automation(
        &b,
        CreateAutomation {
            name: "Archive Done".into(),
            trigger: "card_moved".into(),
            conditions: Some(serde_json::json!({ "to_list_id": done }).to_string()),
            actions: Some(r#"[{"type":"archive"}]"#.into()),
            ..Default::default()
        },
    )
    .unwrap();

    let c = make_card(&s, &todo, "Terminer ça");
    s.move_card(&c.id, MoveCard { list_id: Some(done.clone()), ..Default::default() }).unwrap();
    let got = s.get_card(&c.id).unwrap();
    assert!(got.archived, "card must be archived by automation");
}

#[test]
fn due_soon_sweep_runs_actions() {
    let s = test_service();
    let (b, l) = board_with_list(&s);
    let label = s.create_label(&b, CreateLabel { name: "à-suivre".into(), color: "#0f0".into() }).unwrap();
    s.create_automation(
        &b,
        CreateAutomation {
            name: "Rappel échéance".into(),
            trigger: "card_due_soon".into(),
            conditions: Some(r#"{"days": 2}"#.into()),
            actions: Some(serde_json::json!([{ "type": "label", "label_id": label.id }]).to_string()),
            ..Default::default()
        },
    )
    .unwrap();

    use krello_core::time::add_days;
    let soon = add_days(krello_core::time::today(), 1).to_string();
    let c = make_card(&s, &l, "Échéance proche");
    s.update_card(&c.id, UpdateCard { due: Some(soon), ..Default::default() }).unwrap();

    let far = add_days(krello_core::time::today(), 10).to_string();
    let c2 = make_card(&s, &l, "Échéance lointaine");
    s.update_card(&c2.id, UpdateCard { due: Some(far), ..Default::default() }).unwrap();

    let touched = s.run_scheduled_sweep().unwrap();
    assert_eq!(touched, 1);

    let detail = s.card_detail(&c.id).unwrap();
    assert_eq!(detail["labels"].as_array().unwrap().len(), 1);
    let detail2 = s.card_detail(&c2.id).unwrap();
    assert_eq!(detail2["labels"].as_array().unwrap().len(), 0);
}

#[test]
fn button_automation_targets_by_conditions() {
    let s = test_service();
    let (b, l1) = board_with_list(&s);
    let l2 = s.create_list(&b, CreateList { title: "Revue".into(), ..Default::default() }).unwrap().id;
    make_card(&s, &l1, "dans l1");
    let in_l2 = make_card(&s, &l2, "dans l2");
    s.create_automation(
        &b,
        CreateAutomation {
            name: "Pousser en revue".into(),
            trigger: "button".into(),
            conditions: Some(serde_json::json!({ "list_id": l2 }).to_string()),
            actions: Some(r#"[{"type":"priority","value":"urgent"}]"#.into()),
            ..Default::default()
        },
    )
    .unwrap();

    let autos = s.list_automations(&b).unwrap();
    let touched = s.run_automation(&autos[0].id).unwrap();
    assert_eq!(touched, 1);
    let got = s.get_card(&in_l2.id).unwrap();
    assert_eq!(got.priority, "urgent");
}

#[test]
fn invalid_trigger_rejected() {
    let s = test_service();
    let (b, _l) = board_with_list(&s);
    let err = s.create_automation(
        &b,
        CreateAutomation {
            name: "Mauvaise règle".into(),
            trigger: "quantum".into(),
            ..Default::default()
        },
    );
    assert!(err.is_err());
}

#[test]
fn automation_undo_removes_rule() {
    let s = test_service();
    let (b, _l) = board_with_list(&s);
    let auto = s
        .create_automation(
            &b,
            CreateAutomation {
                name: "Règle".into(),
                trigger: "card_created".into(),
                ..Default::default()
            },
        )
        .unwrap();
    s.undo().unwrap();
    assert!(s.list_automations(&b).unwrap().is_empty());
    s.redo().unwrap();
    assert_eq!(s.list_automations(&b).unwrap().len(), 1);
    let _ = auto;
}
