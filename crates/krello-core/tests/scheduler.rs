mod common;

use common::{board_with_list, make_card, test_service};
use krello_core::models::*;
use krello_core::scheduler;

fn setup_board_with_deps() -> (krello_core::service::Service, String, String, Vec<String>) {
    let s = test_service();
    let (b, l) = board_with_list(&s);
    let a = make_card(&s, &l, "Analyse");
    let bb = make_card(&s, &l, "Développement");
    let c = make_card(&s, &l, "Tests");
    s.update_card(&a.id, UpdateCard { duration_days: Some(2.0), ..Default::default() }).unwrap();
    s.update_card(&bb.id, UpdateCard { duration_days: Some(3.0), ..Default::default() }).unwrap();
    s.update_card(&c.id, UpdateCard { duration_days: Some(1.0), ..Default::default() }).unwrap();
    (s, b, l, vec![a.id, bb.id, c.id])
}

fn add_fs(s: &krello_core::service::Service, board: &str, pred: &str, succ: &str, lag: Option<f64>) {
    s.add_dependency(
        board,
        CreateDependency {
            predecessor_id: pred.into(),
            successor_id: succ.into(),
            kind: "FS".into(),
            lag_days: lag,
        },
    )
    .unwrap();
}

#[test]
fn cpm_critical_path_chain() {
    let (s, b, _l, ids) = setup_board_with_deps();
    let (a, bb, c) = (ids[0].clone(), ids[1].clone(), ids[2].clone());
    add_fs(&s, &b, &a, &bb, None);
    add_fs(&s, &b, &bb, &c, None);

    let summary = s.schedule_for_board(&b).unwrap();
    let find = |id: &str| summary.cards.iter().find(|e| e.card_id == id).unwrap();
    let ea = find(&a);
    let eb = find(&bb);
    let ec = find(&c);
    assert_eq!(ea.es.as_deref(), Some("2026-11-09"));
    assert_eq!(ea.ef.as_deref(), Some("2026-11-10")); // 2 working days
    assert_eq!(eb.es.as_deref(), Some("2026-11-11")); // next working day
    assert_eq!(eb.ef.as_deref(), Some("2026-11-13")); // 11,12,13 (Fri)
    assert_eq!(ec.es.as_deref(), Some("2026-11-16")); // Monday
    assert_eq!(ec.ef.as_deref(), Some("2026-11-16"));
    assert_eq!(summary.finish.as_deref(), Some("2026-11-16"));
    for e in [&ea, &eb, &ec] {
        assert!(e.is_critical, "expected critical: {:?}", e);
        assert_eq!(e.total_float_days, Some(0.0));
    }
}

#[test]
fn weekend_gap_skipped_by_fs_dependency() {
    let (s, b, _l, ids) = setup_board_with_deps();
    let (a, bb) = (ids[0].clone(), ids[1].clone());
    s.update_card(&a, UpdateCard { duration_days: Some(5.0), ..Default::default() }).unwrap();
    add_fs(&s, &b, &a, &bb, None);
    let summary = s.schedule_for_board(&b).unwrap();
    let ea = summary.cards.iter().find(|e| e.card_id == a).unwrap();
    let eb = summary.cards.iter().find(|e| e.card_id == bb).unwrap();
    // Mon 09 → Fri 13 finish; successor starts Monday 16 (weekend skipped)
    assert_eq!(ea.ef.as_deref(), Some("2026-11-13"));
    assert_eq!(eb.es.as_deref(), Some("2026-11-16"));
}

#[test]
fn holiday_calendar_shifts_schedule() {
    let s = test_service();
    let cal = s
        .create_calendar(CreateCalendar {
            name: "FR 2026".into(),
            holidays: Some(r#"["2026-11-11"]"#.into()), // Armistice (Wednesday)
            ..Default::default()
        })
        .unwrap();
    let board = s
        .create_board(CreateBoard {
            title: "Avec férié".into(),
            calendar_id: Some(cal.id.clone()),
            start_date: Some("2026-11-09".into()),
            ..Default::default()
        })
        .unwrap();
    let list = s.create_list(&board.id, CreateList { title: "L".into(), ..Default::default() }).unwrap();
    let a = make_card(&s, &list.id, "A");
    let bb = make_card(&s, &list.id, "B");
    s.update_card(&a.id, UpdateCard { duration_days: Some(3.0), ..Default::default() }).unwrap();
    add_fs(&s, &board.id, &a.id, &bb.id, None);

    let summary = s.schedule_for_board(&board.id).unwrap();
    let ea = summary.cards.iter().find(|e| e.card_id == a.id).unwrap();
    let eb = summary.cards.iter().find(|e| e.card_id == bb.id).unwrap();
    // A: Mon 09, Tue 10, Wed 11 holiday → Thu 12 = 3rd working day
    assert_eq!(ea.ef.as_deref(), Some("2026-11-12"), "11-11 holiday must be skipped");
    assert_eq!(eb.es.as_deref(), Some("2026-11-13"));
}

#[test]
fn ss_dependency_and_lag() {
    let (s, b, _l, ids) = setup_board_with_deps();
    let (a, bb) = (ids[0].clone(), ids[1].clone());
    s.add_dependency(&b, CreateDependency {
        predecessor_id: a.clone(),
        successor_id: bb.clone(),
        kind: "SS".into(),
        lag_days: Some(2.0),
    }).unwrap();
    let summary = s.schedule_for_board(&b).unwrap();
    let ea = summary.cards.iter().find(|e| e.card_id == a).unwrap();
    let eb = summary.cards.iter().find(|e| e.card_id == bb).unwrap();
    assert_eq!(ea.es.as_deref(), Some("2026-11-09"));
    // SS + 2 working days → starts 2026-11-11
    assert_eq!(eb.es.as_deref(), Some("2026-11-11"));
}

#[test]
fn cycle_detection_returns_cycle_path() {
    let (s, b, _l, ids) = setup_board_with_deps();
    let (a, bb, c) = (ids[0].clone(), ids[1].clone(), ids[2].clone());
    add_fs(&s, &b, &a, &bb, None);
    add_fs(&s, &b, &bb, &c, None);
    let err = s.add_dependency(&b, CreateDependency {
        predecessor_id: c.clone(),
        successor_id: a.clone(),
        kind: "FS".into(),
        lag_days: None,
    });
    match err {
        Err(krello_core::Error::Cycle(path)) => {
            assert!(path.contains(&a) && path.contains(&c), "cycle path: {path}");
        }
        other => panic!("expected cycle error, got: {other:?}"),
    }
}

#[test]
fn fixed_work_duration_derived_from_work() {
    let s = test_service();
    let (b, l) = board_with_list(&s);
    let t = make_card(&s, &l, "Tâche charge");
    // 16h of work at 8h/day → 2 working days
    s.update_card(&t.id, UpdateCard {
        task_type: Some("fixed_work".into()),
        work_hours: Some(16.0),
        ..Default::default()
    }).unwrap();
    let summary = s.schedule_for_board(&b).unwrap();
    let e = summary.cards.iter().find(|e| e.card_id == t.id).unwrap();
    assert_eq!(e.es.as_deref(), Some("2026-11-09"));
    assert_eq!(e.ef.as_deref(), Some("2026-11-10"));
}

#[test]
fn milestone_has_zero_span() {
    let s = test_service();
    let (b, l) = board_with_list(&s);
    let m = make_card(&s, &l, "Kickoff");
    s.update_card(&m.id, UpdateCard { is_milestone: Some(true), ..Default::default() }).unwrap();
    let summary = s.schedule_for_board(&b).unwrap();
    let e = summary.cards.iter().find(|e| e.card_id == m.id).unwrap();
    assert_eq!(e.es, e.ef);
}

#[test]
fn schedule_survives_undo_of_dependency() {
    let (s, b, _l, ids) = setup_board_with_deps();
    let (a, bb) = (ids[0].clone(), ids[1].clone());
    add_fs(&s, &b, &a, &bb, None);
    let after = s.schedule_for_board(&b).unwrap();
    let eb = after.cards.iter().find(|e| e.card_id == bb).unwrap();
    assert_eq!(eb.es.as_deref(), Some("2026-11-11"));

    s.undo().unwrap(); // removes dependency
    let before = s.schedule_for_board(&b).unwrap();
    let eb = before.cards.iter().find(|e| e.card_id == bb).unwrap();
    assert_eq!(eb.es.as_deref(), Some("2026-11-09"), "no dependency → ASAP at project start");

    s.redo().unwrap();
    let restored = s.schedule_for_board(&b).unwrap();
    let eb = restored.cards.iter().find(|e| e.card_id == bb).unwrap();
    assert_eq!(eb.es.as_deref(), Some("2026-11-11"), "redo restores dependency");
}

#[test]
fn baseline_comparison_reports_variance() {
    let s = test_service();
    let (b, l) = board_with_list(&s);
    let t = make_card(&s, &l, "Tâche planifiée");
    s.update_card(&t.id, UpdateCard {
        start: Some("2026-11-09".into()),
        due: Some("2026-11-12".into()),
        duration_days: Some(4.0),
        ..Default::default()
    }).unwrap();

    let baseline = s.create_baseline(&b, "v1").unwrap();
    s.update_card(&t.id, UpdateCard { due: Some("2026-11-16".into()), ..Default::default() }).unwrap();

    let diffs = s.compare_baseline(&baseline.id).unwrap();
    let d = diffs.iter().find(|d| d.card_id == t.id).unwrap();
    assert_eq!(d.due_baseline.as_deref(), Some("2026-11-12"));
    assert_eq!(d.due_now.as_deref(), Some("2026-11-16"));
    assert_eq!(d.delta_due_days, Some(4), "slipped 4 calendar days");
}

#[test]
fn working_calendar_math() {
    let cal = scheduler::WorkingCal {
        weekends: [6, 0].into_iter().collect(),
        holidays: [].into_iter().collect(),
        hours_per_day: 8.0,
    };
    use chrono::NaiveDate;
    let mon = NaiveDate::from_ymd_opt(2026, 11, 9).unwrap();
    let fri = NaiveDate::from_ymd_opt(2026, 11, 13).unwrap();
    assert_eq!(cal.add_working(fri, 1), NaiveDate::from_ymd_opt(2026, 11, 16).unwrap());
    assert_eq!(cal.add_working(mon, -1), NaiveDate::from_ymd_opt(2026, 11, 6).unwrap());
    assert_eq!(cal.count_between(mon, fri), 4);
    assert!(cal.is_working(mon));
    assert!(!cal.is_working(NaiveDate::from_ymd_opt(2026, 11, 14).unwrap())); // saturday
}

#[test]
fn wbs_codes_assigned_in_list_order() {
    let (s, b, _l, ids) = setup_board_with_deps();
    let (a, _bb, c) = (ids[0].clone(), ids[1].clone(), ids[2].clone());
    s.schedule_for_board(&b).unwrap();
    let a_card = s.get_card(&a).unwrap();
    let c_card = s.get_card(&c).unwrap();
    assert_eq!(a_card.wbs, "1");
    assert_eq!(c_card.wbs, "3");
}
