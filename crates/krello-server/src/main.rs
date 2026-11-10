//! Krello local server binary: REST + WebSocket + static frontend, 127.0.0.1 only.

use std::path::PathBuf;
use std::sync::Arc;

use tower_http::services::ServeDir;

use krello_core::db::Db;
use krello_core::service::Service;
use krello_server::{build_router, default_db_path, frontend_dir, make_state};

const DEFAULT_PORT: u16 = 8787;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "krello_server=info,tower_http=warn".into()),
        )
        .init();

    // Args: --db PATH --port N --token T --frontend DIR --open
    let mut db_path = default_db_path();
    let mut port = DEFAULT_PORT;
    let mut open_browser = false;
    let mut token = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--db" => db_path = args.next().map(PathBuf::from).unwrap_or(db_path),
            "--port" => {
                port = args.next().and_then(|p| p.parse().ok()).unwrap_or(port);
            }
            "--open" => open_browser = true,
            "--token" => token = args.next(),
            "--frontend" => {
                if let Some(dir) = args.next() {
                    std::env::set_var("KRELLO_FRONTEND_DIR", dir);
                }
            }
            other => {
                eprintln!(
                    "Usage: krello [--db PATH] [--port N] [--token T] [--frontend DIR] [--open]\nArgument inconnu: {other}"
                );
                std::process::exit(2);
            }
        }
    }

    let db = Db::open(&db_path).unwrap_or_else(|e| {
        eprintln!("Impossible d'ouvrir la base {}: {e}", db_path.display());
        std::process::exit(1);
    });
    let attachments_dir = db_path
        .parent()
        .map(|p| p.join("attachments"))
        .unwrap_or_else(|| PathBuf::from("attachments"));
    let service = Arc::new(Service::new(db, attachments_dir));

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    tracing::info!("Krello démarré sur http://{addr}/ (base: {})", db_path.display());

    // Scheduled automations sweep every minute.
    {
        let service = service.clone();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(std::time::Duration::from_secs(60));
            loop {
                ticker.tick().await;
                let svc = service.clone();
                let _ = tokio::task::spawn_blocking(move || svc.run_scheduled_sweep()).await;
            }
        });
    }

    let state = make_state(service, token, None);
    let mut app = build_router(state);

    match frontend_dir() {
        Some(dir) => {
            tracing::info!("frontend servi depuis {}", dir.display());
            app = app.fallback_service(ServeDir::new(dir).append_index_html_on_directories(true));
        }
        None => {
            tracing::warn!("frontend introuvable — lancez depuis la racine du projet ou utilisez --frontend");
            app = app.fallback(|| async {
                (
                    axum::http::StatusCode::NOT_FOUND,
                    "Interface non trouvée. Définissez KRELLO_FRONTEND_DIR ou lancez depuis la racine du projet.",
                )
            });
        }
    }

    if open_browser {
        let url = format!("http://{addr}/");
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(400)).await;
            let _ = open::that(url);
        });
    }

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap_or_else(|e| {
        eprintln!("Port {port} indisponible: {e}");
        std::process::exit(1);
    });
    axum::serve(listener, app).await.unwrap();
}
