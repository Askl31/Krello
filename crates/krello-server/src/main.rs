//! Krello local server binary: REST + WebSocket + static frontend, 127.0.0.1 only.

use std::path::PathBuf;
use std::sync::Arc;


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
    // Sous-commandes CLI (`krello task add …`) : exécutées hors serveur, puis exit.
    if krello_server::cli::try_run_cli() {
        return;
    }

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
                    "Usage: krello [--db PATH] [--port N] [--token T] [--frontend DIR] [--open] [--no-ai]\n       krello task add --title \"…\" [--list \"…\"] [--board \"…\"] [--db PATH]\nArgument inconnu: {other}"
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

    // Automatic database backups: checked on the same cadence, fires only when
    // the configured interval (default 24h) has elapsed and a directory is set.
    {
        let service = service.clone();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(std::time::Duration::from_secs(60));
            loop {
                ticker.tick().await;
                let svc = service.clone();
                let _ = tokio::task::spawn_blocking(move || svc.maybe_auto_backup()).await;
            }
        });
    }

    let state = make_state(service, token, ai);
    if let Some(dir) = frontend_dir() {
        tracing::info!("frontend disponible sur disque ({}) et embarqué", dir.display());
    } else {
        tracing::info!("frontend servi depuis les assets embarqués");
    }
    let app = build_router(state)
        // `any` (pas `get`) : un POST/PUT/DELETE vers une route inconnue doit
        // retomber sur le fallback (404 JSON pour /api/*), pas sur un 405.
        .fallback_service(axum::routing::any(krello_server::fallback_handler));

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
