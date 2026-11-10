//! Integration tests over the axum router using tower oneshot (no sockets).

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

use krello_server::test_krello_app;

async fn body_json(body: Body) -> Value {
    let bytes = body.collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap_or(Value::Null)
}

async fn json_req(app: &Router, method: &str, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    let req = if let Some(b) = body {
        builder = builder.header("content-type", "application/json");
        builder.body(Body::from(b.to_string())).unwrap()
    } else {
        builder.body(Body::empty()).unwrap()
    };
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let json = body_json(res.into_body()).await;
    (status, json)
}

#[tokio::test]
async fn health_and_board_flow() {
    let app = test_krello_app();
    let (status, health) = json_req(&app, "GET", "/api/v1/health", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(health["ok"], true);
    assert!(health["ai"].is_object(), "ai status present");

    // create board
    let (status, board) = json_req(
        &app,
        "POST",
        "/api/v1/boards",
        Some(serde_json::json!({ "title": "Integration", "start_date": "2026-11-09" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let board_id = board["id"].as_str().unwrap().to_string();

    // list
    let (status, list) = json_req(
        &app,
        "POST",
        &format!("/api/v1/boards/{board_id}/lists"),
        Some(serde_json::json!({ "title": "À faire" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let list_id = list["id"].as_str().unwrap().to_string();

    // card (5 working days)
    let (status, card) = json_req(
        &app,
        "POST",
        &format!("/api/v1/boards/{board_id}/cards"),
        Some(serde_json::json!({ "title": "Carte API", "list_id": list_id, "duration_days": 5.0 })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let card_id = card["id"].as_str().unwrap().to_string();

    // move
    let (status, moved) = json_req(
        &app,
        "POST",
        &format!("/api/v1/cards/{card_id}/move"),
        Some(serde_json::json!({ "position": 0.0 })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(moved["position"], 0.0);

    // schedule recompute (5 working days from Mon 2026-11-09 → Fri 2026-11-13)
    let (status, schedule) = json_req(&app, "POST", &format!("/api/v1/boards/{board_id}/schedule"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(schedule["finish"], "2026-11-13");

    // search
    let (status, hits) = json_req(&app, "GET", &format!("/api/v1/search?q=carte&board_id={board_id}"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(hits.as_array().unwrap().len(), 1);

    // export markdown
    let req = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/boards/{board_id}/export?format=md"))
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let md = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(md.contains("# Integration"));

    // undo → card deleted? undo removes last op (export doesn't undo; move does)
    let (status, undone) = json_req(&app, "POST", "/api/v1/undo", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(undone["undone"], "Déplacement de la carte « Carte API »");
}

#[tokio::test]
async fn dependency_cycle_conflict_mapping() {
    let app = test_krello_app();
    let (_, board) = json_req(&app, "POST", "/api/v1/boards", Some(serde_json::json!({"title": "Cycles"}))).await;
    let board_id = board["id"].as_str().unwrap().to_string();
    let (_, list) = json_req(&app, "POST", &format!("/api/v1/boards/{board_id}/lists"), Some(serde_json::json!({"title": "L"}))).await;
    let list_id = list["id"].as_str().unwrap().to_string();
    let mut ids = Vec::new();
    for i in 0..3 {
        let (_, c) = json_req(&app, "POST", &format!("/api/v1/boards/{board_id}/cards"), Some(serde_json::json!({"title": format!("T{i}"), "list_id": list_id}))).await;
        ids.push(c["id"].as_str().unwrap().to_string());
    }
    for w in ids.windows(2) {
        let (status, _) = json_req(
            &app,
            "POST",
            &format!("/api/v1/boards/{board_id}/dependencies"),
            Some(serde_json::json!({"predecessor_id": w[0], "successor_id": w[1], "type": "FS"})),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
    }
    let (status, err) = json_req(
        &app,
        "POST",
        &format!("/api/v1/boards/{board_id}/dependencies"),
        Some(serde_json::json!({"predecessor_id": ids[2], "successor_id": ids[0], "type": "FS"})),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(err["error"], "dependency_cycle");
}

#[tokio::test]
async fn validation_and_not_found_mapping() {
    let app = test_krello_app();
    let (status, err) = json_req(&app, "POST", "/api/v1/boards", Some(serde_json::json!({"title": "  "}))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(err["error"], "validation");

    let (status, err) = json_req(&app, "GET", "/api/v1/cards/nope", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(err["error"], "not_found");
}

#[tokio::test]
async fn ai_status_endpoint_shape() {
    let app = test_krello_app();
    let (status, ai) = json_req(&app, "GET", "/api/v1/ai/status", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(ai["available"].is_boolean());
    assert!(ai["backend"].is_string());
}

#[tokio::test]
async fn trello_import_endpoint() {
    let app = test_krello_app();
    let fixture = serde_json::json!({
        "name": "Trello via API",
        "lists": [{ "id": "tl1", "name": "Colonne", "pos": 1, "closed": false }],
        "labels": [],
        "cards": [{ "id": "c1", "name": "Carte importée", "desc": "", "idList": "tl1", "idLabels": [], "pos": 1, "closed": false, "customFieldItems": [] }],
        "checklists": [],
        "customFields": [],
        "actions": []
    });
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/import/trello")
        .header("content-type", "application/json")
        .body(Body::from(fixture.to_string()))
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
}
