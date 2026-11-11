//! Automatic backups: settings validation, VACUUM INTO snapshot readability,
//! rotation (keep), interval gating via maybe_auto_backup.

mod common;

use common::*;
use krello_core::db::Db;
use krello_core::service::settings::DEFAULT_INTERVAL_HOURS;

fn tmp_backup_dir(name: &str) -> String {
    let d = std::env::temp_dir().join(format!("krello-backup-test-{}-{name}", std::process::id()));
    std::fs::remove_dir_all(&d).ok();
    d.to_string_lossy().into_owned()
}

#[test]
fn backup_creates_readable_db_and_rotates_to_keep() {
    let s = test_service();
    let (_board_id, _list_id) = board_with_list(&s); // au moins un board dans la copie
    let dir = tmp_backup_dir("rotate");
    s.set_backup_settings(Some(dir.clone()), Some(6), Some(2)).unwrap();
    let st = s.backup_settings().unwrap();
    assert_eq!(st.backup_dir.as_deref(), Some(dir.as_str()));
    assert_eq!(st.interval_hours, 6);
    assert_eq!(st.keep, 2);

    let p1 = s.run_backup_now().unwrap();
    assert!(p1.exists(), "le fichier de sauvegarde doit exister");
    // La copie est une base Krello ouvrable, avec le schéma et les données.
    let db = Db::open(&p1).unwrap();
    let n: i64 = db
        .conn
        .query_row("SELECT COUNT(*) FROM boards", [], |r| r.get(0))
        .unwrap();
    assert!(n >= 1, "le board de test doit être dans la sauvegarde");

    // Rotation: 3 backups avec keep=2 → il n'en reste que 2.
    std::thread::sleep(std::time::Duration::from_millis(1100));
    s.run_backup_now().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(1100));
    s.run_backup_now().unwrap();
    let remaining: usize = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .filter(|e| {
            e.file_name()
                .to_str()
                .map(|n| n.starts_with("krello-backup-") && n.ends_with(".db"))
                .unwrap_or(false)
        })
        .count();
    assert_eq!(remaining, 2, "keep=2 doit purger les plus anciennes");

    let st2 = s.backup_settings().unwrap();
    assert!(st2.last_backup_at.is_some() && st2.last_backup_file.is_some());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn backup_requires_dir_and_rejects_unusable_path() {
    let s = test_service();
    // Sans répertoire configuré → erreur explicite.
    assert!(s.run_backup_now().is_err());
    // Chemin impossible à créer (sous /proc) → Validation.
    let err = s
        .set_backup_settings(Some("/proc/krello-nope/backups".into()), None, None)
        .unwrap_err();
    assert!(
        matches!(err, krello_core::Error::Validation(_)),
        "attendu Validation, reçu {err:?}"
    );
    // Désactivation: répertoire vide → backup_dir repasse à None.
    s.set_backup_settings(Some(String::new()), None, None).unwrap();
    assert_eq!(s.backup_settings().unwrap().backup_dir, None);
    assert!(s.maybe_auto_backup().unwrap().is_none(), "désactivé → rien");
}

#[test]
fn maybe_auto_backup_respects_interval() {
    let s = test_service();
    let dir = tmp_backup_dir("interval");
    s.set_backup_settings(Some(dir), Some(24), None).unwrap();
    // Première passe: jamais sauvegardé → fire.
    assert!(s.maybe_auto_backup().unwrap().is_some());
    // Immédiatement après: intervalle non écoulé → rien.
    assert!(s.maybe_auto_backup().unwrap().is_none());
    // Valeurs par défaut cohérentes.
    let st = s.backup_settings().unwrap();
    assert_eq!(st.interval_hours, DEFAULT_INTERVAL_HOURS);
    std::fs::remove_dir_all(st.backup_dir.unwrap()).ok();
}
