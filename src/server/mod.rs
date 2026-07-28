pub mod middlewares;
mod mount;

use std::net::SocketAddr;

use axum::http::header;
use axum::{middleware, Router};
use tower_http::cors::CorsLayer;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::sensitive_headers::SetSensitiveRequestHeadersLayer;
use tower_http::trace::TraceLayer;

use crate::provider::Provider;
use crate::shared::errdef::Error;

/// Builds the application router with the global middleware stack applied in
/// the same order the Go server used.
pub fn router(provider: &Provider) -> Router {
    mount::mount(provider)
        .fallback(route_not_found)
        .layer(middleware::from_fn(
            middlewares::request_context::request_context,
        ))
        .layer(TraceLayer::new_for_http())
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
        .layer(SetSensitiveRequestHeadersLayer::new([
            header::AUTHORIZATION,
        ]))
        .layer(CorsLayer::permissive())
}

/// Unmatched routes get the same error envelope as everything else.
async fn route_not_found() -> Error {
    Error::not_found("route not found")
}

pub async fn serve(provider: Provider) -> anyhow::Result<()> {
    let addr = SocketAddr::from(([0, 0, 0, 0], provider.config.server.port));
    let app = router(&provider);

    let listener = tokio::net::TcpListener::bind(addr).await?;

    tracing::info!(%addr, "http server listening");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;

    Ok(())
}

/// Stops accepting connections on Ctrl+C or on the SIGTERM Docker sends.
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install the Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install the SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("shutdown signal received");
}
