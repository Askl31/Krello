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

    // Args: --db PATH --port N --token T --frontend DIR --open --no-ai
    let mut db_path = default_db_path();
    let mut port = DEFAULT_PORT;
    let mut open_browser = false;
    let mut token = None;
    let mut disable_ai = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--db" => db_path = args.next().map(PathBuf::from).unwrap_or(db_path),
            "--port" => {
                port = args.next().and_then(|p| p.parse().ok()).unwrap_or(port);
            }
            "--open" => open_browser = true,
            "--token" => token = args.next(),
            "--no-ai" => disable_ai = true,
            "--frontend" => {
                if let Some(dir) = args.next() {
                    std::env::set_var("KRELLO_FRONTEND_DIR", dir);
                }
            }
            other => {
                eprintln!(
                    "Usage: krello [--db PATH] [--port N] [--token T] [--frontend DIR] [--open] [--no-ai]\nArgument inconnu: {other}"
                );
                std::process::exit(2);
            }
        }
    }
    let ai: Option<std::sync::Arc<dyn krello_core::ai::provider::AiProvider>> =
        if disable_ai || std::env::var("KRELLO_NO_AI").is_ok() {
            Some(Arc::new(krello_core::ai::provider::UnavailableProvider {
                reason: "disabled".into(),
            }))
        } else {
            None
        };

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

    let state = make_state(service, token, ai);
    let mut app = build_router(state);

    match frontend_dir() {
        Some(dir) => {
            tracing::info!("frontend servi depuis {}", dir.display());
            app = app.fallback_service(ServeDir::new(dir).append_index_html_on_directories(true));
        }
        None => {
            // Assets embarqués à la compilation (include_dir) — le binaire est autonome.
            tracing::info!("frontend servi depuis les assets embarqués");
            app = app.fallback_service(axum::routing::get(krello_server::embedded_frontend_handler));
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
