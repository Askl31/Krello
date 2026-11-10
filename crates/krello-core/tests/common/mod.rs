use krello_core::db::Db;
use krello_core::models::*;
use krello_core::service::Service;
use std::path::PathBuf;

pub fn test_service() -> Service {
    let db = Db::open_memory().unwrap();
    Service::new(db, std::env::temp_dir().join(format!("krello-test-{}", std::process::id())))
}

pub fn board_with_list(s: &Service) -> (String, String) {
    let b = s
        .create_board(CreateBoard {
            title: "Projet Test".into(),
            description: "desc".into(),
            start_date: Some("2026-11-09".into()),
            ..Default::default()
        })
        .unwrap();
    let l = s
        .create_list(&b.id, CreateList { title: "À faire".into(), ..Default::default() })
        .unwrap();
    (b.id, l.id)
}

pub fn make_card(s: &Service, list_id: &str, title: &str) -> Card {
    s.create_card(
        "unused",
        CreateCard {
            title: title.into(),
            list_id: list_id.into(),
            ..Default::default()
        },
    )
    .unwrap_or_else(|_| {
        // fetch board id from list
        let l = s.get_list(list_id).unwrap();
        s.create_card(
            &l.board_id,
            CreateCard {
                title: title.into(),
                list_id: list_id.into(),
                ..Default::default()
            },
        )
        .unwrap()
    })
}

#[allow(dead_code)]
pub fn dummy() -> PathBuf {
    PathBuf::from("/tmp")
}
