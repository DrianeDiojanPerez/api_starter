mod docs;
pub mod middlewares;
mod mount;

use std::net::SocketAddr;
use std::sync::Arc;

use axum::http::header;
use axum::{middleware, Router};
use tower_http::cors::CorsLayer;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::sensitive_headers::SetSensitiveRequestHeadersLayer;
use tower_http::trace::TraceLayer;

use crate::module::iam;
use crate::package::auth::Auth;
use crate::package::errdef::Error;
use crate::package::rbac::Engine;
use crate::provider::Provider;

pub struct Modules {
    pub auth: Arc<dyn Auth>,
    pub rbac: Arc<dyn Engine>,
    pub iam: iam::Services,
}

impl From<&Provider> for Modules {
    fn from(provider: &Provider) -> Self {
        Self {
            auth: provider.auth.clone(),
            rbac: provider.rbac.clone(),
            iam: provider.iam.clone(),
        }
    }
}

pub fn router(modules: &Modules) -> Router {
    mount::mount(modules)
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

async fn route_not_found() -> Error {
    Error::not_found("route not found")
}

pub async fn serve(provider: Provider) -> anyhow::Result<()> {
    let addr = SocketAddr::from(([0, 0, 0, 0], provider.config.server.port));
    let app = router(&Modules::from(&provider));

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
