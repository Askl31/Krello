//! Sous-commandes CLI exécutées hors serveur : `krello task add …`.
//!
//! Utilisées depuis un Terminal, un script, ou en repli par les raccourcis
//! Siri/Spotlight (deep link `krello://` indisponible). Écrit directement dans
//! la base SQLite (WAL : coexiste avec une app Krello ouverte). Le binaire
//! `krello` (serveur) et le binaire de l'app Tauri interceptent tous deux
//! `task` comme premier argument via [`try_run_cli`].

use std::io::Read;
use std::path::PathBuf;

use krello_core::db::Db;
use krello_core::models::CreateCard;
use krello_core::service::Service;

use crate::default_db_path;

/// Si le premier argument est `task`, exécute la sous-commande puis quitte le
/// process. Retourne `true` si une sous-commande a été reconnue (ne revient
/// jamais dans ce cas).
pub fn try_run_cli() -> bool {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(|a| a.as_str()) != Some("task") {
        return false;
    }
    std::process::exit(run_task(&args[1..]));
}

/// `task add --title "…" [--list "…"] [--board "…"] [--db PATH]`.
/// `--title -` lit le titre sur stdin. Code de sortie : 0 ok, 1 erreur, 2 usage.
fn run_task(rest: &[String]) -> i32 {
    if rest.first().map(|s| s.as_str()) != Some("add") {
        eprintln!(
            "Usage: krello task add --title \"…\" [--list \"…\"] [--board \"…\"] [--db PATH]\n       (--title - lit le titre sur stdin)"
        );
        return 2;
    }
    let mut title: Option<String> = None;
    let mut list: Option<String> = None;
    let mut board: Option<String> = None;
    let mut db: Option<PathBuf> = None;
    let mut it = rest[1..].iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--title" => title = it.next().cloned(),
            "--list" => list = it.next().cloned(),
            "--board" => board = it.next().cloned(),
            "--db" => db = it.next().map(PathBuf::from),
            other => {
                eprintln!("Argument inconnu: {other}");
                return 2;
            }
        }
    }
    let title = match title.as_deref() {
        Some("-") => {
            let mut buf = String::new();
            if std::io::stdin().read_to_string(&mut buf).is_err() {
                eprintln!("Lecture stdin impossible");
                return 1;
            }
            buf.trim().to_string()
        }
        Some(t) => t.trim().to_string(),
        None => {
            eprintln!("--title requis (ou --title - pour lire stdin)");
            return 2;
        }
    };
    if title.is_empty() {
        eprintln!("Le titre ne peut pas être vide");
        return 2;
    }
    let db_path = db.unwrap_or_else(default_db_path);
    let db = match Db::open(&db_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Ouverture de la base {} impossible: {e}", db_path.display());
            return 1;
        }
    };
    let attachments_dir = db_path
        .parent()
        .map(|p| p.join("attachments"))
        .unwrap_or_else(|| PathBuf::from("attachments"));
    let service = Service::new(db, attachments_dir);
    let resolved = match &list {
        Some(l) if !l.trim().is_empty() => service.find_list(l, board.as_deref()),
        _ => service.find_default_list(),
    };
    let (list, board) = match resolved {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };
    match service.create_card(
        &board.id,
        CreateCard {
            title: title.clone(),
            list_id: list.id.clone(),
            ..Default::default()
        },
    ) {
        Ok(card) => {
            println!(
                "OK {} « {} » → liste « {} » (board « {} ») — base {}",
                card.id,
                card.title,
                list.title,
                board.title,
                db_path.display()
            );
            0
        }
        Err(e) => {
            eprintln!("{e}");
            1
        }
    }
}
