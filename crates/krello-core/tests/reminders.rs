//! Effective reminders: one event per (reminder, due), dedupe via the
//! `reminders_sent` map, rearm on due/reminders change, skip
//! archived/done/template/no-due cards.

mod common;

use common::*;
use krello_core::events::Event;
use krello_core::models::*;
use krello_core::service::Service;

fn rel(offset: &str) -> Reminder {
    Reminder { kind: "relative".into(), offset: Some(offset.into()), at: None }
}

fn abs(at: &str) -> Reminder {
    Reminder { kind: "absolute".into(), offset: None, at: Some(at.into()) }
}

#[test]
fn reminder_fires_once_then_stays_silent() {
    let s = test_service();
    let (_board_id, list_id) = board_with_list(&s);
    let c = make_card(&s, &list_id, "rappel");
    // Échéance passée avec rappel 1 j avant → moment dépassé → doit partir.
    s.update_card(
        &c.id,
        UpdateCard {
            due: Some("2020-01-02T12:00:00".into()),
            reminders: Some(vec![rel("1d")]),
            ..Default::default()
        },
    )
    .unwrap();

    let mut rx = s.subscribe();
    assert_eq!(s.fire_due_reminders().unwrap(), 1, "un seul rappel émis");
    assert!(matches!(rx.try_recv(), Ok(se) if matches!(se.event, Event::ReminderDue { .. })));
    assert_eq!(s.fire_due_reminders().unwrap(), 0, "pas de doublon au second sweep");
    let card = s.get_card(&c.id).unwrap();
    assert!(!card.reminders_sent.is_null(), "bookkeeping posé");
}

#[test]
fn reminder_skips_archived_done_template_and_no_due() {
    let s = test_service();
    let (_board_id, list_id) = board_with_list(&s);

    let done = make_card(&s, &list_id, "terminée");
    s.update_card(&done.id, UpdateCard {
        due: Some("2020-01-02".into()),
        reminders: Some(vec![rel("1d")]),
        due_complete: Some(true),
        ..Default::default()
    }).unwrap();

    let archived = make_card(&s, &list_id, "archivée");
    s.update_card(&archived.id, UpdateCard {
        due: Some("2020-01-02".into()),
        reminders: Some(vec![rel("1d")]),
        archived: Some(true),
        ..Default::default()
    }).unwrap();

    let template = make_card(&s, &list_id, "modèle");
    s.set_template(&template.id, SetTemplate { is_template: true, template_description: None }).unwrap();
    s.update_card(&template.id, UpdateCard {
        due: Some("2020-01-02".into()),
        reminders: Some(vec![rel("1d")]),
        ..Default::default()
    }).unwrap();

    let no_due = make_card(&s, &list_id, "sans échéance");
    s.update_card(&no_due.id, UpdateCard {
        reminders: Some(vec![rel("1h")]),
        ..Default::default()
    }).unwrap();

    assert_eq!(s.fire_due_reminders().unwrap(), 0, "aucune de ces cartes ne doit sonner");
}

#[test]
fn future_reminder_not_fired_and_due_change_rearms() {
    let s = test_service();
    let (_board_id, list_id) = board_with_list(&s);
    let c = make_card(&s, &list_id, "futur");

    // Échéance très lointaine + rappel 1 j → moment futur → silencieux.
    s.update_card(&c.id, UpdateCard {
        due: Some("2999-01-02".into()),
        reminders: Some(vec![rel("1d")]),
        ..Default::default()
    }).unwrap();
    assert_eq!(s.fire_due_reminders().unwrap(), 0);

    // Changer la due réarme les rappels (reminders_sent repasse à NULL).
    s.update_card(&c.id, UpdateCard {
        due: Some("2020-01-02".into()),
        ..Default::default()
    }).unwrap();
    let card = s.get_card(&c.id).unwrap();
    assert!(card.reminders_sent.is_null(), "changement de due → rappels réarmés");
    assert_eq!(s.fire_due_reminders().unwrap(), 1);
}

#[test]
fn multiple_reminders_fire_independently() {
    let s = test_service();
    let (_board_id, list_id) = board_with_list(&s);
    let c = make_card(&s, &list_id, "double");
    // Due passée + deux rappels (1 j et 1 h avant) → les deux moments sont dépassés.
    s.update_card(&c.id, UpdateCard {
        due: Some("2020-01-02T12:00:00".into()),
        reminders: Some(vec![rel("1d"), rel("1h")]),
        ..Default::default()
    }).unwrap();

    assert_eq!(s.fire_due_reminders().unwrap(), 2, "les deux rappels partent");
    assert_eq!(s.fire_due_reminders().unwrap(), 0, "aucun doublon");

    // Changer la due réarme TOUS les rappels.
    s.update_card(&c.id, UpdateCard {
        due: Some("2020-01-03T12:00:00".into()),
        ..Default::default()
    }).unwrap();
    assert_eq!(s.fire_due_reminders().unwrap(), 2, "réarmement complet");
}

#[test]
fn mixed_relative_absolute_partial_future() {
    let s = test_service();
    let (_board_id, list_id) = board_with_list(&s);
    let c = make_card(&s, &list_id, "mixte");
    // Rappel 1 relatif (moment dépassé) + rappel 2 absolu (moment futur).
    s.update_card(&c.id, UpdateCard {
        due: Some("2020-01-02".into()),
        reminders: Some(vec![rel("1d"), abs("2999-01-01T00:00:00")]),
        ..Default::default()
    }).unwrap();

    assert_eq!(s.fire_due_reminders().unwrap(), 1, "seul le rappel dépassé part");
    assert_eq!(s.fire_due_reminders().unwrap(), 0, "le futur reste armé, pas de doublon");

    // Remplacer la liste réarme tout : le rappel absolu devient dépassé.
    s.update_card(&c.id, UpdateCard {
        reminders: Some(vec![rel("1d"), abs("2020-01-01T09:00:00")]),
        ..Default::default()
    }).unwrap();
    assert_eq!(s.fire_due_reminders().unwrap(), 2);
}

#[test]
fn reminder_moment_parsing() {
    // Relatifs : N avant l'échéance.
    let m = Service::reminder_moment(&rel("1d"), "2020-01-02").unwrap();
    assert_eq!(m.to_string(), "2020-01-01 00:00:00");
    let m2 = Service::reminder_moment(&rel("2h"), "2020-01-02T10:00:00").unwrap();
    assert_eq!(m2.to_string(), "2020-01-02 08:00:00");
    let m3 = Service::reminder_moment(&rel("15m"), "2020-01-02T10:30:00").unwrap();
    assert_eq!(m3.to_string(), "2020-01-02 10:15:00");
    let m5 = Service::reminder_moment(&rel("1w"), "2020-01-09").unwrap();
    assert_eq!(m5.to_string(), "2020-01-02 00:00:00");
    // Absolu : datetime ISO directement (due ignorée).
    let m4 = Service::reminder_moment(&abs("2020-01-01T09:00:00"), "2020-01-02").unwrap();
    assert_eq!(m4.to_string(), "2020-01-01 09:00:00");
    // Formats inconnus → None.
    assert!(Service::reminder_moment(&Reminder { kind: "bidon".into(), offset: None, at: None }, "2020-01-02").is_none());
    assert!(Service::reminder_moment(&rel("99z"), "2020-01-02").is_none());
    assert!(Service::reminder_moment(&rel("1d"), "pas-une-date").is_none());
}

#[test]
fn invalid_reminders_are_rejected() {
    let s = test_service();
    let (_board_id, list_id) = board_with_list(&s);
    let c = make_card(&s, &list_id, "validation");
    assert!(s.update_card(&c.id, UpdateCard {
        reminders: Some(vec![rel("99z")]),
        ..Default::default()
    }).is_err(), "offset inconnu rejeté");
    assert!(s.update_card(&c.id, UpdateCard {
        reminders: Some(vec![abs("pas-une-date")]),
        ..Default::default()
    }).is_err(), "date absurde rejetée");
    assert!(s.update_card(&c.id, UpdateCard {
        reminders: Some(vec![Reminder { kind: "flottant".into(), offset: None, at: None }]),
        ..Default::default()
    }).is_err(), "kind inconnu rejeté");
}

#[test]
fn sweep_includes_reminders_in_total() {
    let s = test_service();
    let (_board_id, list_id) = board_with_list(&s);
    let c = make_card(&s, &list_id, "via sweep");
    s.update_card(&c.id, UpdateCard {
        due: Some("2020-01-02".into()),
        reminders: Some(vec![rel("1d")]),
        ..Default::default()
    }).unwrap();
    // Le sweep minute (scheduled + reminders) compte le rappel émis.
    assert!(s.run_scheduled_sweep().unwrap() >= 1, "le sweep doit inclure les rappels");
}
