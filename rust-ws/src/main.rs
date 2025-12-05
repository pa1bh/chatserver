mod handlers;
mod protocol;
mod state;
mod utils;

use std::net::SocketAddr;

use axum::{routing::get, Router};
use tokio::net::TcpListener;
use tracing::info;

use handlers::ws_handler;
use state::AppState;

#[tokio::main]
async fn main() {
    // Default: no logging (warn level). Use RUST_LOG=info or RUST_LOG=debug for output.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_target(false)
        .init();

    let port = std::env::var("WS_PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(3001);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    let state = AppState::new();

    let app = Router::new()
        .route("/", get(ws_handler))
        .with_state(state);

    let listener = TcpListener::bind(addr).await.expect("bind to address");
    info!(port, "Rust WS server start");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .expect("start ws server");

    info!("Server shut down gracefully");
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => info!("Received Ctrl+C, shutting down..."),
        _ = terminate => info!("Received SIGTERM, shutting down..."),
    }
}
