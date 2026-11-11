//! Time tracking: `time_spent_minutes` banks increments (never replaced),
//! `tracking_started_at` "" stops the session (→ NULL), undo/redo restore both.

mod common;

use common::*;
use krello_core::models::*;
use krello_core::time::now_rfc3339;

#[test]
fn tracking_banks_minutes_and_stops_cleanly() {
    let s = test_service();
    let (_board_id, list_id) = board_with_list(&s);
    let c = make_card(&s, &list_id, "trackée");

    // Banque d'un incrément: 0 + 25 = 25
    let after = s
        .update_card(&c.id, UpdateCard { time_spent_minutes: Some(25), ..Default::default() })
        .unwrap();
    assert_eq!(after.time_spent_minutes, 25, "incrément banqué");
    // Cumulé: +5 → 30
    let again = s
        .update_card(&c.id, UpdateCard { time_spent_minutes: Some(5), ..Default::default() })
        .unwrap();
    assert_eq!(again.time_spent_minutes, 30, "incrément cumulé");

    // "" = arrêter le suivi → NULL
    let stopped = s
        .update_card(&c.id, UpdateCard { tracking_started_at: Some(String::new()), ..Default::default() })
        .unwrap();
    assert_eq!(stopped.tracking_started_at, None, "suivi arrêté (\"\" → NULL)");
    assert_eq!(stopped.time_spent_minutes, 30, "les minutes banquées restent");

    // Une session en cours démarre puis s'arrête: banque 0 (moins d'une minute)
    let running = s
        .update_card(&c.id, UpdateCard { tracking_started_at: Some(now_rfc3339()), ..Default::default() })
        .unwrap();
    assert!(running.tracking_started_at.is_some(), "session en cours");
    let stopped2 = s
        .update_card(&c.id, UpdateCard { tracking_started_at: Some(String::new()), ..Default::default() })
        .unwrap();
    assert_eq!(stopped2.tracking_started_at, None, "session arrêtée");
    assert_eq!(stopped2.time_spent_minutes, 30, "moins d'une minute → +0 min");

    // Undo/redo : chaque opération a son inverse exact. Les 4 dernières
    // opérations : stop2, start, stop1 (no-ops d'état), banque +5 (25→30).
    s.undo().unwrap(); // stop2  (30 → 30)
    s.undo().unwrap(); // start  (30 → 30)
    s.undo().unwrap(); // stop1  (30 → 30)
    let undone = s.get_card(&c.id).unwrap();
    assert_eq!(undone.time_spent_minutes, 30, "les no-ops n'ont pas d'effet");
    s.undo().unwrap(); // banque +5 (30 → 25)
    let undone2 = s.get_card(&c.id).unwrap();
    assert_eq!(undone2.time_spent_minutes, 25, "l'undo retire la banque +5");
    s.redo().unwrap();
    let redone = s.get_card(&c.id).unwrap();
    assert_eq!(redone.time_spent_minutes, 30, "le redo rebanque");
}
