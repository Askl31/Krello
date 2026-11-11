//! Application native Krello (Tauri v2).
//!
//! Lance le serveur axum in-process sur un port éphémère (127.0.0.1 uniquement)
//! et ouvre une fenêtre pointant sur l'URL locale — le frontend est servi par le
//! serveur lui-même (assets embarqués). Toute la logique (SQLite, CPM, Apple
//! Intelligence via le helper Swift embarqué) vient de `krello-core`.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::sync::Arc;

use axum::routing::any;
use krello_core::db::Db;
use krello_core::service::Service;
use krello_server::{build_router, fallback_handler, make_state};

fn app_data_dir() -> PathBuf {
    if cfg!(target_os = "macos") {
        std::env::var("HOME")
            .map(|h| PathBuf::from(h).join("Library/Application Support/com.krello.app"))
            .unwrap_or_else(|_| PathBuf::from("."))
    } else if cfg!(target_os = "windows") {
        std::env::var("APPDATA")
            .map(|d| PathBuf::from(d).join("Krello"))
            .unwrap_or_else(|_| PathBuf::from("."))
    } else {
        std::env::var("HOME")
            .map(|h| PathBuf::from(h).join(".local/share/krello"))
            .unwrap_or_else(|_| PathBuf::from("."))
    }
}

#[cfg(target_os = "macos")]
fn setup_dock_icon() {
    use objc2::AllocAnyThread;
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApplication, NSImage};
    use objc2_foundation::NSData;

    let icon_bytes = include_bytes!("../icons/icon.png");

    unsafe {
        let mtm = MainThreadMarker::new_unchecked();
        let app = NSApplication::sharedApplication(mtm);
        let data = NSData::with_bytes(icon_bytes);
        let image = NSImage::initWithData(NSImage::alloc(), &data);
        if let Some(img) = image {
            app.setApplicationIconImage(Some(&img));
        }
    }
}

fn main() {
    #[cfg(target_os = "macos")]
    setup_dock_icon();
    // Le serveur in-process partage ce processus : la fenêtre native existe,
    // on autorise donc le sélecteur de répertoire natif (Finder/Explorer).
    std::env::set_var("KRELLO_NATIVE_DIALOG", "1");
    let data_dir = app_data_dir();
    let db_path = data_dir.join("krello.db");
    let db = Db::open(&db_path).expect("ouverture de la base Krello impossible");
    let service = Arc::new(Service::new(db, data_dir.join("attachments")));

    tauri::Builder::default()
        .setup(move |app| {
            #[cfg(target_os = "macos")]
            setup_dock_icon();

            let handle = app.handle().clone();
            let service_server = Arc::clone(&service);
            let service_sweep = Arc::clone(&service);
            tauri::async_runtime::spawn(async move {
                let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                    .await
                    .expect("bind 127.0.0.1:0");
                let port = listener.local_addr().unwrap().port();
                let url: tauri::Url = format!("http://127.0.0.1:{port}/").parse().unwrap();

                let state = make_state(service_server, None, None);
                let router =
                    build_router(state).fallback_service(any(fallback_handler));

                let webview = tauri::webview::WebviewWindowBuilder::new(
                    &handle,
                    "main",
                    tauri::WebviewUrl::External(url),
                )
                .title("Krello")
                .inner_size(1280.0, 820.0)
                .min_inner_size(940.0, 600.0)
                .build();
                if let Err(e) = webview {
                    tracing::error!("fenêtre: {e}");
                }

                if let Err(e) = axum::serve(listener, router).await {
                    tracing::error!("serveur: {e}");
                }
            });

            // Sweep des automatisations programmées.
            tauri::async_runtime::spawn(async move {
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                    let svc = Arc::clone(&service_sweep);
                    let _ = tokio::task::spawn_blocking(move || svc.run_scheduled_sweep()).await;
                    let svc_backup = Arc::clone(&service_sweep);
                    let _ =
                        tokio::task::spawn_blocking(move || svc_backup.maybe_auto_backup()).await;
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("erreur lors de l'exécution de Krello");
}
