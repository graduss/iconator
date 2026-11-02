use std::{io::IsTerminal, path::PathBuf};

use axum::{Router, routing::get};
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

mod app;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| "info,tower_http=debug".into()),
        )
        .with_ansi(std::io::stdout().is_terminal())
        .init();

    let icons_dir = std::env::var_os("ICONS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/icons")));

    let state = app::AppState { icons_dir };

    let server = Router::new()
        .merge(app::get_server())
        .route("/health", get(|| async { "ok" }))
        .with_state(state)
        .layer(TraceLayer::new_for_http());
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("failed to bind 0.0.0.0:3000");
    tracing::info!("listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, server)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to listen for Ctrl+C")
    };
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to listen for SIGTERM")
            .recv()
            .await;
    };
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("shutting down");
}
