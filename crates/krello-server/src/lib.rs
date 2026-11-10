//! Krello server library: router construction shared by the binary, the
//! Tauri app and the integration tests.

pub mod error;
pub mod routes;
pub mod state;
pub mod ws;

use std::path::PathBuf;
use std::sync::Arc;

use axum::middleware::{self, Next};
use axum::extract::State;
use axum::http::Request;
use axum::response::{IntoResponse, Response};
use tower_http::trace::TraceLayer;

use krello_core::db::Db;
use krello_core::service::Service;
use state::{build_ai_provider, AppState, SharedState};

/// Optional bearer-token guard (localhost is trusted otherwise).
async fn local_guard(
    State(state): State<SharedState>,
    req: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let is_api = req.uri().path().starts_with("/api/v1");
    let is_health = req.uri().path() == "/api/v1/health";
    if let Some(token) = &state.token {
        if is_api && !is_health {
            let ok = req
                .headers()
                .get(axum::http::header::AUTHORIZATION)
                .and_then(|v| v.to_str().ok())
                .map(|v| v == format!("Bearer {token}"))
                .unwrap_or(false);
            if !ok {
                return (
                    axum::http::StatusCode::UNAUTHORIZED,
                    axum::Json(serde_json::json!({ "error": "unauthorized" })),
                )
                    .into_response();
            }
        }
    }
    next.run(req).await
}

/// Full API router (REST + WS) with the local guard, no-cache and tracing.
pub fn build_router(state: SharedState) -> axum::Router {
    async fn no_cache(req: Request<axum::body::Body>, next: Next) -> Response {
        let mut res = next.run(req).await;
        res.headers_mut().insert(
            axum::http::header::CACHE_CONTROL,
            axum::http::HeaderValue::from_static("no-cache"),
        );
        res
    }
    let guarded = routes::router(state.clone())
        .layer(middleware::from_fn_with_state(state, local_guard));
    guarded
        .layer(middleware::from_fn(no_cache))
        .layer(TraceLayer::new_for_http())
}

/// Build the shared application state.
pub fn make_state(
    service: Arc<Service>,
    token: Option<String>,
    ai: Option<Arc<dyn krello_core::ai::provider::AiProvider>>,
) -> SharedState {
    Arc::new(AppState {
        service,
        ai: ai.unwrap_or_else(build_ai_provider),
        token,
    })
}

/// Default on-disk DB location per platform.
pub fn default_db_path() -> PathBuf {
    let home = std::env::var("HOME").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("."));
    if cfg!(target_os = "macos") {
        home.join("Library/Application Support/com.krello.app/krello.db")
    } else if cfg!(target_os = "windows") {
        std::env::var("APPDATA")
            .map(|d| PathBuf::from(d).join("Krello/krello.db"))
            .unwrap_or_else(|_| home.join(".krello/krello.db"))
    } else {
        home.join(".local/share/krello/krello.db")
    }
}

pub fn frontend_dir() -> Option<PathBuf> {
    let candidates = [
        std::env::var("KRELLO_FRONTEND_DIR").ok().map(PathBuf::from),
        Some(PathBuf::from("frontend")),
        Some(PathBuf::from("../frontend")),
        Some(PathBuf::from("../../frontend")),
    ];
    for c in candidates.into_iter().flatten() {
        if c.join("index.html").exists() {
            return Some(c);
        }
    }
    None
}

/// Frontend embarqué dans le binaire (app Tauri, binaire autonome).
pub static FRONTEND_EMBEDDED: include_dir::Dir<'_> =
    include_dir::include_dir!("$CARGO_MANIFEST_DIR/../../frontend");

fn mime_for(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        _ => "application/octet-stream",
    }
}

/// Handler de fallback : sert les assets (disque si disponible en dev, sinon embarqués).
/// Pour les routes SPA (sans extension de fichier), renvoie index.html.
/// Pour les assets manquants (.js, .css, .png, etc.), renvoie 404 au lieu de index.html
/// afin d'éviter les erreurs SyntaxError: Unexpected token '<' dans le navigateur.
pub async fn embedded_frontend_handler(uri: axum::http::Uri) -> Response {
    let raw_path = uri.path().trim_start_matches('/');
    let path = if raw_path.is_empty() { "index.html" } else { raw_path };

    // 1. Essai de lecture directe sur disque si le dossier frontend existe (mode dev / live reload)
    if let Some(dir) = frontend_dir() {
        let disk_path = dir.join(path);
        if disk_path.is_file() {
            if let Ok(bytes) = std::fs::read(&disk_path) {
                let mime = mime_for(path);
                return (
                    [
                        (axum::http::header::CONTENT_TYPE, mime),
                        (axum::http::header::CACHE_CONTROL, "no-cache"),
                    ],
                    bytes,
                )
                    .into_response();
            }
        }
    }

    // 2. Recherche dans les assets embarqués à la compilation
    if let Some(f) = FRONTEND_EMBEDDED.get_file(path) {
        let mime = mime_for(f.path().to_str().unwrap_or(""));
        return (
            [
                (axum::http::header::CONTENT_TYPE, mime),
                (axum::http::header::CACHE_CONTROL, "no-cache"),
            ],
            f.contents().to_vec(),
        )
            .into_response();
    }

    // 3. Si le chemin a une extension (ex: .js, .css, .svg, .png), c'est un asset manquant : 404 !
    // Ne jamais servir index.html pour un asset, sinon le navigateur tente d'exécuter du HTML comme du JS.
    let is_asset = path
        .rsplit('/')
        .next()
        .map(|segment| segment.contains('.'))
        .unwrap_or(false);
    if is_asset && path != "index.html" {
        return (axum::http::StatusCode::NOT_FOUND, "Ressource introuvable").into_response();
    }

    // 4. Fallback SPA pour la navigation (#/b/..., etc.) : servir index.html
    if let Some(dir) = frontend_dir() {
        let disk_index = dir.join("index.html");
        if let Ok(bytes) = std::fs::read(&disk_index) {
            return (
                [
                    (axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8"),
                    (axum::http::header::CACHE_CONTROL, "no-cache"),
                ],
                bytes,
            )
                .into_response();
        }
    }

    if let Some(f) = FRONTEND_EMBEDDED.get_file("index.html") {
        return (
            [
                (axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8"),
                (axum::http::header::CACHE_CONTROL, "no-cache"),
            ],
            f.contents().to_vec(),
        )
            .into_response();
    }

    (axum::http::StatusCode::NOT_FOUND, "index.html introuvable").into_response()
}

/// Test router: in-memory DB, AI unavailable.
pub fn test_krello_app() -> axum::Router {
    let db = Db::open_memory().expect("in-memory db");
    let service = Arc::new(Service::new(
        db,
        std::env::temp_dir().join(format!("krello-server-test-{}", std::process::id())),
    ));
    let state = make_state(
        service,
        None,
        Some(Arc::new(krello_core::ai::provider::UnavailableProvider {
            reason: "test".into(),
        })),
    );
    build_router(state)
}
