//! Deep links `krello://` (ajout rapide via Siri/Spotlight sur macOS) et
//! résolution de la liste cible par id ou par nom.
//!
//! Format supporté : `krello://task/new?list=<id|nom>&board=<id|nom>&title=<texte>&confirm=1`
//! - `list` / `board` : id exact d'abord, sinon nom (insensible à la casse) ;
//!   sans `list`, la première liste du premier board est utilisée.
//! - `title` : titre passé dans l'URL (percent-encodé). À défaut, le contenu
//!   du fichier « inbox » Siri (`<data_dir>/siri-inbox.txt`, lu puis supprimé).
//! - `confirm=1` : ne crée pas la carte, renvoie `NeedsInput` avec le titre
//!   pré-rempli (la modale Krello demande confirmation).

use std::path::PathBuf;

use rusqlite::OptionalExtension;
use serde::Serialize;

use crate::error::{Error, Result};
use crate::models::{Board, Card, CreateCard, List};
use crate::service::Service;
use crate::undo::{query_board, row_list};

/// Nom du fichier inbox déposé par les raccourcis Siri/Spotlight.
pub const INBOX_FILE: &str = "siri-inbox.txt";

#[derive(Debug, Clone, PartialEq)]
pub struct DeepLink {
    /// Chemin de l'action, ex. `task/new`.
    pub action: String,
    pub list: Option<String>,
    pub board: Option<String>,
    pub title: Option<String>,
    pub confirm: bool,
}

/// Décodage percent-encoding (`%XX`) + `+` → espace.
pub fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hex = &s[i + 1..i + 3];
                if let Ok(v) = u8::from_str_radix(hex, 16) {
                    out.push(v);
                    i += 3;
                    continue;
                }
                out.push(b'%');
                i += 1;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Parse une URL `krello://…`. Erreur `Validation` si le scheme est faux.
pub fn parse_deep_link(url: &str) -> Result<DeepLink> {
    let rest = url
        .strip_prefix("krello://")
        .ok_or_else(|| Error::Validation(format!("scheme krello:// attendu, reçu « {url} »")))?;
    let (path, query) = match rest.split_once('?') {
        Some((p, q)) => (p, q),
        None => (rest, ""),
    };
    let action = path.trim_matches('/').to_string();
    if action.is_empty() {
        return Err(Error::Validation(
            "action manquante (ex. krello://task/new?list=Courses)".into(),
        ));
    }
    let mut link = DeepLink {
        action,
        list: None,
        board: None,
        title: None,
        confirm: false,
    };
    for pair in query.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        let v = percent_decode(v);
        match k {
            "list" => link.list = Some(v),
            "board" => link.board = Some(v),
            "title" => link.title = Some(v),
            "confirm" => link.confirm = v == "1" || v.eq_ignore_ascii_case("true"),
            _ => {}
        }
    }
    Ok(link)
}

/// Résultat du traitement d'un deep link (sérialisé en JSON pour l'API).
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DeepLinkOutcome {
    Created { card: Card, list: List, board: Board },
    NeedsInput { board: Board, list: List, title: String },
}

impl Service {
    /// Répertoire de données de l'app (parent du dossier `attachments`).
    pub fn data_dir(&self) -> PathBuf {
        self.attachments_dir
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| self.attachments_dir.clone())
    }

    /// Fichier inbox déposé par les raccourcis Siri/Spotlight.
    pub fn siri_inbox_path(&self) -> PathBuf {
        self.data_dir().join(INBOX_FILE)
    }

    /// Résout une liste par id, sinon par nom (insensible à la casse), dans le
    /// board indiqué si fourni, sinon dans tous les boards actifs (premier
    /// board par date de création, première liste par position).
    pub fn find_list(&self, list_ref: &str, board_ref: Option<&str>) -> Result<(List, Board)> {
        // 1) id exact
        if let Ok(l) = self.get_list(list_ref) {
            if !l.archived {
                let db = self.lock_db()?;
                if let Some(b) = query_board(&db.conn, &l.board_id)? {
                    return Ok((l, b));
                }
            }
        }
        // 2) nom de liste (+ board optionnel)
        let db = self.lock_db()?;
        let mut sql = String::from(
            "SELECT l.* FROM lists l JOIN boards b ON b.id = l.board_id \
             WHERE l.archived = 0 AND b.archived = 0 AND lower(l.title) = lower(?1)",
        );
        let mut params: Vec<String> = vec![list_ref.to_string()];
        if let Some(br) = board_ref {
            sql.push_str(" AND (b.id = ?2 OR lower(b.title) = lower(?2))");
            params.push(br.to_string());
        }
        sql.push_str(" ORDER BY b.created_at, l.position");
        let mut stmt = db.conn.prepare_cached(&sql)?;
        let found = stmt
            .query_map(rusqlite::params_from_iter(params.iter()), row_list)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        drop(stmt);
        let list = found
            .into_iter()
            .next()
            .ok_or_else(|| Error::NotFound(format!("liste « {list_ref} » introuvable")))?;
        let board = query_board(&db.conn, &list.board_id)?
            .ok_or_else(|| Error::NotFound(format!("board {} introuvable", list.board_id)))?;
        Ok((list, board))
    }

    /// Première liste du premier board actif — cible par défaut d'un deep link
    /// sans paramètre `list`.
    pub fn find_default_list(&self) -> Result<(List, Board)> {
        let (board_id, list_id) = {
            let db = self.lock_db()?;
            let board_id: Option<String> = db
                .conn
                .query_row(
                    "SELECT id FROM boards WHERE archived = 0 ORDER BY created_at LIMIT 1",
                    [],
                    |r| r.get(0),
                )
                .optional()?;
            let board_id = board_id.ok_or_else(|| Error::NotFound("aucun board actif".into()))?;
            let list_id: Option<String> = db
                .conn
                .query_row(
                    "SELECT id FROM lists WHERE board_id = ?1 AND archived = 0 ORDER BY position LIMIT 1",
                    [&board_id],
                    |r| r.get(0),
                )
                .optional()?;
            let list_id = list_id.ok_or_else(|| Error::NotFound("aucune liste active".into()))?;
            (board_id, list_id)
        }; // le guard db est libéré ici
        let list = self.get_list(&list_id)?;
        let board = {
            let db = self.lock_db()?;
            query_board(&db.conn, &board_id)?
                .ok_or_else(|| Error::NotFound(format!("board {board_id}")))?
        };
        Ok((list, board))
    }

    /// Traite un deep link `krello://task/new…` : résout la liste, récupère le
    /// titre (paramètre `title` ou fichier inbox), crée la carte — ou renvoie
    /// `NeedsInput` si le titre est vide ou si `confirm=1`.
    pub fn handle_deep_link(&self, url: &str) -> Result<DeepLinkOutcome> {
        let link = parse_deep_link(url)?;
        if link.action != "task/new" && link.action != "task/add" {
            return Err(Error::Validation(format!(
                "action inconnue « {} » (attendu : task/new)",
                link.action
            )));
        }
        let (list, board) = match &link.list {
            Some(l) if !l.trim().is_empty() => self.find_list(l, link.board.as_deref())?,
            _ => self.find_default_list()?,
        };
        // Titre : paramètre URL, sinon fichier inbox Siri (lu puis supprimé).
        let mut title = link.title.clone().unwrap_or_default();
        if title.trim().is_empty() {
            let inbox = self.siri_inbox_path();
            if let Ok(content) = std::fs::read_to_string(&inbox) {
                let _ = std::fs::remove_file(&inbox);
                title = content;
            }
        }
        let title = title.trim().to_string();
        if title.is_empty() || link.confirm {
            return Ok(DeepLinkOutcome::NeedsInput { board, list, title });
        }
        let card = self.create_card(
            &board.id,
            CreateCard {
                title,
                list_id: list.id.clone(),
                ..Default::default()
            },
        )?;
        Ok(DeepLinkOutcome::Created { card, list, board })
    }
}
