mod common;

use common::{board_with_list, make_card, test_service};
use krello_core::models::*;

#[test]
fn card_created_trigger_moves_new_cards() {
    let s = test_service();
    let (b, inbox) = board_with_list(&s);
    let _todo = s.create_list(&b, CreateList { title: "À faire".into(), ..Default::default() }).unwrap();

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

// ---------- v3: cross-board, templates, cascade, run log, new triggers ----------

fn second_board(s: &krello_core::service::Service) -> (String, String) {
    let b = s
        .create_board(CreateBoard { title: "Board Cible".into(), ..Default::default() })
        .unwrap();
    let l = s
        .create_list(&b.id, CreateList { title: "Suivi".into(), ..Default::default() })
        .unwrap();
    (b.id, l.id)
}

#[test]
fn cross_board_copy_on_move_copies_labels_and_checklists_not_dates() {
    let s = test_service();
    let (b, inbox) = board_with_list(&s);
    let done = s.create_list(&b, CreateList { title: "Terminé".into(), ..Default::default() }).unwrap().id;
    let (b2, suivi) = second_board(&s);

    let label = s.create_label(&b, CreateLabel { name: "urgent".into(), color: "#f00".into() }).unwrap();
    s.create_automation(
        &b,
        CreateAutomation {
            name: "Copier vers Board Cible".into(),
            trigger: "card_moved".into(),
            conditions: Some(serde_json::json!({ "to_list_id": done }).to_string()),
            actions: Some(serde_json::json!([
                { "type": "copy", "list_id": suivi, "title": "[copie] {{card.title}}" }
            ]).to_string()),
            ..Default::default()
        },
    )
    .unwrap();

    let c = make_card(&s, &inbox, "Tâche importante");
    s.attach_label(&c.id, &label.id).unwrap();
    let cl = s.create_checklist(&c.id, CreateChecklist { title: "Étapes".into(), position: None }).unwrap();
    s.create_checklist_item(&cl.id, CreateChecklistItem { title: "étape 1".into(), ..Default::default() }).unwrap();
    let due = krello_core::time::add_days(krello_core::time::today(), 3).to_string();
    s.update_card(&c.id, UpdateCard { due: Some(due.clone()), ..Default::default() }).unwrap();

    s.move_card(&c.id, MoveCard { list_id: Some(done), ..Default::default() }).unwrap();

    // Source card untouched, still in board A.
    let src = s.get_card(&c.id).unwrap();
    assert_eq!(src.board_id, b);

    // Copy exists in board B.
    let cards_b = s.list_cards(&b2, false).unwrap();
    assert_eq!(cards_b.len(), 1, "one copy in target board");
    let copy = &cards_b[0];
    assert_eq!(copy.title, "[copie] Tâche importante");
    assert_eq!(copy.due, None, "dates are never copied");
    let detail = s.card_detail(&copy.id).unwrap();
    let labels = detail["labels"].as_array().unwrap();
    assert_eq!(labels.len(), 1);
    assert_eq!(labels[0]["name"], "urgent", "label mapped by name onto target board");
    assert_ne!(labels[0]["id"], label.id, "target label is a new row");
    let cls = detail["checklists"].as_array().unwrap();
    assert_eq!(cls.len(), 1);
    assert_eq!(cls[0]["items"].as_array().unwrap().len(), 1, "checklist deep-copied");

    // The label was auto-created on board B.
    let labels_b = s.list_labels(&b2).unwrap();
    assert_eq!(labels_b.len(), 1);
    assert_eq!(labels_b[0].name, "urgent");
}

#[test]
fn create_card_action_in_other_board_with_template_and_cascade() {
    let s = test_service();
    let (b, inbox) = board_with_list(&s);
    let done = s.create_list(&b, CreateList { title: "Terminé".into(), ..Default::default() }).unwrap().id;
    let (b2, suivi) = second_board(&s);
    let label_b2 = s.create_label(&b2, CreateLabel { name: "auto".into(), color: "#00f".into() }).unwrap();

    // Rule on A: when a card lands in "Terminé", create a follow-up card in board B.
    s.create_automation(
        &b,
        CreateAutomation {
            name: "Créer suivi cross-board".into(),
            trigger: "card_moved".into(),
            conditions: Some(serde_json::json!({ "to_list_id": done }).to_string()),
            actions: Some(serde_json::json!([
                { "type": "create_card", "list_id": suivi,
                  "title": "Suivi de « {{card.title}} » ({{board.title}})",
                  "priority": "high" }
            ]).to_string()),
            ..Default::default()
        },
    )
    .unwrap();
    // Rule on B: every card created there gets the "auto" label (proves cascade).
    s.create_automation(
        &b2,
        CreateAutomation {
            name: "Label auto".into(),
            trigger: "card_created".into(),
            actions: Some(serde_json::json!([{ "type": "label", "label_id": label_b2.id }]).to_string()),
            ..Default::default()
        },
    )
    .unwrap();

    let c = make_card(&s, &inbox, "Livrer le rapport");
    s.move_card(&c.id, MoveCard { list_id: Some(done), ..Default::default() }).unwrap();

    let cards_b = s.list_cards(&b2, false).unwrap();
    assert_eq!(cards_b.len(), 1);
    let created = &cards_b[0];
    assert_eq!(created.title, "Suivi de « Livrer le rapport » (Projet Test)");
    assert_eq!(created.priority, "high");
    let detail = s.card_detail(&created.id).unwrap();
    let labels = detail["labels"].as_array().unwrap();
    assert_eq!(labels.len(), 1, "cascade: card_created rule on board B fired");
    assert_eq!(labels[0]["name"], "auto");
}

#[test]
fn cross_board_cascade_is_depth_bounded() {
    let s = test_service();
    let (b, la) = board_with_list(&s);
    let (b2, lb) = second_board(&s);
    // A: card_created -> copy to B. B: card_created -> copy to A. Must terminate.
    s.create_automation(
        &b,
        CreateAutomation {
            name: "A->B".into(),
            trigger: "card_created".into(),
            actions: Some(serde_json::json!([{ "type": "copy", "list_id": lb }]).to_string()),
            ..Default::default()
        },
    )
    .unwrap();
    s.create_automation(
        &b2,
        CreateAutomation {
            name: "B->A".into(),
            trigger: "card_created".into(),
            actions: Some(serde_json::json!([{ "type": "copy", "list_id": la }]).to_string()),
            ..Default::default()
        },
    )
    .unwrap();

    make_card(&s, &la, "origine");
    let n_a = s.list_cards(&b, false).unwrap().len();
    let n_b = s.list_cards(&b2, false).unwrap().len();
    // MAX_DEPTH=4: rules fire at depths 0,2,4 on A and 1,3 on B → 3 cards each, then stop.
    assert_eq!(n_a, 3, "depth-bounded copies on A");
    assert_eq!(n_b, 3, "depth-bounded copies on B");
}

#[test]
fn card_archived_and_card_commented_triggers_fire() {
    let s = test_service();
    let (b, l) = board_with_list(&s);
    s.create_automation(
        &b,
        CreateAutomation {
            name: "Archivage".into(),
            trigger: "card_archived".into(),
            actions: Some(r#"[{"type":"comment","text":"Archivée le {{date}}"}]"#.into()),
            ..Default::default()
        },
    )
    .unwrap();
    s.create_automation(
        &b,
        CreateAutomation {
            name: "Commentaire".into(),
            trigger: "card_commented".into(),
            actions: Some(r#"[{"type":"priority","value":"low"}]"#.into()),
            ..Default::default()
        },
    )
    .unwrap();

    let c = make_card(&s, &l, "Carte test");
    s.add_comment(&c.id, "hello").unwrap();
    assert_eq!(s.get_card(&c.id).unwrap().priority, "low", "card_commented fired");
    s.update_card(&c.id, UpdateCard { archived: Some(true), ..Default::default() }).unwrap();
    let comments = s.list_comments(&c.id).unwrap();
    assert!(comments.iter().any(|cm| cm.body.contains("Archivée le")), "card_archived fired");
}

#[test]
fn checklist_item_checked_trigger_fires() {
    let s = test_service();
    let (b, l) = board_with_list(&s);
    s.create_automation(
        &b,
        CreateAutomation {
            name: "Item cochée".into(),
            trigger: "checklist_item_checked".into(),
            actions: Some(r#"[{"type":"priority","value":"urgent"}]"#.into()),
            ..Default::default()
        },
    )
    .unwrap();
    let c = make_card(&s, &l, "Avec checklist");
    let cl = s.create_checklist(&c.id, CreateChecklist { title: "Choses".into(), position: None }).unwrap();
    let item = s.create_checklist_item(&cl.id, CreateChecklistItem { title: "étape".into(), ..Default::default() }).unwrap();
    assert_eq!(s.get_card(&c.id).unwrap().priority, "none");
    s.update_checklist_item(&item.id, UpdateChecklistItem { checked: Some(true), ..Default::default() }).unwrap();
    assert_eq!(s.get_card(&c.id).unwrap().priority, "urgent");
}

#[test]
fn card_unlabeled_trigger_fires() {
    let s = test_service();
    let (b, l) = board_with_list(&s);
    let label = s.create_label(&b, CreateLabel { name: "bug".into(), color: "#f00".into() }).unwrap();
    s.create_automation(
        &b,
        CreateAutomation {
            name: "Retrait bug".into(),
            trigger: "card_unlabeled".into(),
            actions: Some(r#"[{"type":"clear_due"}]"#.into()),
            ..Default::default()
        },
    )
    .unwrap();
    let c = make_card(&s, &l, "Carte bug");
    s.attach_label(&c.id, &label.id).unwrap();
    let due = krello_core::time::add_days(krello_core::time::today(), 5).to_string();
    s.update_card(&c.id, UpdateCard { due: Some(due), ..Default::default() }).unwrap();
    s.detach_label(&c.id, &label.id).unwrap();
    assert_eq!(s.get_card(&c.id).unwrap().due, None, "clear_due applied on unlabel");
}

#[test]
fn scheduled_daily_runs_once_per_day() {
    let s = test_service();
    let (b, l) = board_with_list(&s);
    s.create_automation(
        &b,
        CreateAutomation {
            name: "Quotidien".into(),
            trigger: "scheduled_daily".into(),
            conditions: Some(r#"{"time": "00:00"}"#.into()),
            actions: Some(r#"[{"type":"priority","value":"low"}]"#.into()),
            ..Default::default()
        },
    )
    .unwrap();
    make_card(&s, &l, "c1");
    let t1 = s.run_scheduled_sweep().unwrap();
    assert_eq!(t1, 1);
    let t2 = s.run_scheduled_sweep().unwrap();
    assert_eq!(t2, 0, "second sweep same day must be a no-op (dedupe)");
}

#[test]
fn card_due_overdue_trigger_and_dedupe() {
    let s = test_service();
    let (b, l) = board_with_list(&s);
    let label = s.create_label(&b, CreateLabel { name: "retard".into(), color: "#f00".into() }).unwrap();
    s.create_automation(
        &b,
        CreateAutomation {
            name: "Échéance atteinte".into(),
            trigger: "card_due".into(),
            conditions: Some("{}".into()),
            actions: Some(serde_json::json!([{ "type": "label", "label_id": label.id }]).to_string()),
            ..Default::default()
        },
    )
    .unwrap();
    let past = krello_core::time::add_days(krello_core::time::today(), -2).to_string();
    let c = make_card(&s, &l, "En retard");
    s.update_card(&c.id, UpdateCard { due: Some(past), ..Default::default() }).unwrap();
    let t1 = s.run_scheduled_sweep().unwrap();
    assert_eq!(t1, 1);
    let detail = s.card_detail(&c.id).unwrap();
    assert_eq!(detail["labels"].as_array().unwrap().len(), 1);
    let t2 = s.run_scheduled_sweep().unwrap();
    assert_eq!(t2, 0, "no duplicate run the same day");
}

#[test]
fn move_action_cross_board_drops_board_scoped_data() {
    let s = test_service();
    let (b, inbox) = board_with_list(&s);
    let done = s.create_list(&b, CreateList { title: "Terminé".into(), ..Default::default() }).unwrap().id;
    let (b2, suivi) = second_board(&s);
    let label = s.create_label(&b, CreateLabel { name: "urgent".into(), color: "#f00".into() }).unwrap();
    s.create_automation(
        &b,
        CreateAutomation {
            name: "Déplacer cross-board".into(),
            trigger: "card_moved".into(),
            conditions: Some(serde_json::json!({ "to_list_id": done }).to_string()),
            actions: Some(serde_json::json!([{ "type": "move", "list_id": suivi }]).to_string()),
            ..Default::default()
        },
    )
    .unwrap();
    let c = make_card(&s, &inbox, "À transférer");
    s.attach_label(&c.id, &label.id).unwrap();
    s.move_card(&c.id, MoveCard { list_id: Some(done), ..Default::default() }).unwrap();
    let got = s.get_card(&c.id).unwrap();
    assert_eq!(got.board_id, b2, "card moved to other board");
    assert_eq!(got.list_id, suivi);
    let detail = s.card_detail(&c.id).unwrap();
    assert_eq!(detail["labels"].as_array().unwrap().len(), 0, "labels dropped cross-board");
    assert!(s.list_cards(&b, false).unwrap().is_empty());
    assert_eq!(s.list_cards(&b2, false).unwrap().len(), 1);
}

#[test]
fn button_rule_supports_new_actions_and_run_log_records() {
    let s = test_service();
    let (b, l) = board_with_list(&s);
    let label = s.create_label(&b, CreateLabel { name: "x".into(), color: "#123".into() }).unwrap();
    let field = s.create_field_def(&b, CreateFieldDef { name: "Client".into(), kind: "text".into(), options: None }).unwrap();
    let auto = s
        .create_automation(
            &b,
            CreateAutomation {
                name: "Nettoyage".into(),
                trigger: "button".into(),
                conditions: Some(serde_json::json!({ "list_id": l }).to_string()),
                actions: Some(serde_json::json!([
                    { "type": "unlabel", "label_id": label.id },
                    { "type": "clear_due" },
                    { "type": "set_title", "value": "[OK] {{card.title}}" },
                    { "type": "set_field", "field_id": field.id, "value": "Acme" },
                    { "type": "comment", "text": "Traité le {{date}}" },
                    { "type": "check_item", "item_title": "étape 1" },
                    { "type": "unarchive" }
                ]).to_string()),
                ..Default::default()
            },
        )
        .unwrap();

    let c = make_card(&s, &l, "À nettoyer");
    s.attach_label(&c.id, &label.id).unwrap();
    let due = krello_core::time::add_days(krello_core::time::today(), 1).to_string();
    s.update_card(&c.id, UpdateCard { due: Some(due), archived: Some(true), ..Default::default() }).unwrap();
    let cl = s.create_checklist(&c.id, CreateChecklist { title: "S".into(), position: None }).unwrap();
    let item = s.create_checklist_item(&cl.id, CreateChecklistItem { title: "étape 1".into(), ..Default::default() }).unwrap();

    // Button targets exclude archived cards; unarchive first manually.
    s.update_card(&c.id, UpdateCard { archived: Some(false), ..Default::default() }).unwrap();
    let touched = s.run_automation(&auto.id).unwrap();
    assert_eq!(touched, 1);

    let got = s.get_card(&c.id).unwrap();
    assert_eq!(got.title, "[OK] À nettoyer");
    assert_eq!(got.due, None);
    let detail = s.card_detail(&c.id).unwrap();
    assert_eq!(detail["labels"].as_array().unwrap().len(), 0);
    let item_after = s.list_checklists(&c.id).unwrap().into_iter().find(|(cl2, _)| cl2.id == cl.id).unwrap().1;
    assert!(item_after.iter().find(|i| i.id == item.id).unwrap().checked, "check_item checked the existing item");
    let comments = s.list_comments(&c.id).unwrap();
    assert!(comments.iter().any(|cm| cm.body.starts_with("Traité le")));

    // Run log.
    let runs = s.list_automation_runs(&auto.id, 50).unwrap();
    assert_eq!(runs.len(), 1);
    assert!(runs[0].ok);
    assert_eq!(runs[0].trigger, "button");
    assert_eq!(runs[0].card_id.as_deref(), Some(c.id.as_str()));
    let board_runs = s.list_board_automation_runs(&b, 50).unwrap();
    assert!(board_runs.iter().any(|r| r.automation_id == auto.id));
}

#[test]
fn invalid_action_and_missing_fields_rejected() {
    let s = test_service();
    let (b, _l) = board_with_list(&s);
    let e1 = s.create_automation(
        &b,
        CreateAutomation {
            name: "bad".into(),
            trigger: "card_created".into(),
            actions: Some(r#"[{"type":"explode"}]"#.into()),
            ..Default::default()
        },
    );
    assert!(e1.is_err(), "unknown action rejected");
    let e2 = s.create_automation(
        &b,
        CreateAutomation {
            name: "bad2".into(),
            trigger: "card_created".into(),
            actions: Some(r#"[{"type":"create_card","list_id":"x"}]"#.into()),
            ..Default::default()
        },
    );
    assert!(e2.is_err(), "create_card without title rejected");
    let e3 = s.create_automation(
        &b,
        CreateAutomation {
            name: "bad3".into(),
            trigger: "card_created".into(),
            conditions: Some("[1,2]".into()),
            ..Default::default()
        },
    );
    assert!(e3.is_err(), "non-object conditions rejected");
}
