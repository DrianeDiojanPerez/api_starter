use axum::routing::get;
use axum::Router;
use serde::Serialize;

use crate::package::response;

#[derive(Debug, Serialize)]
pub struct Health {
    pub status: &'static str,
    pub version: &'static str,
}

pub fn routes() -> Router {
    Router::new().route("/v1/healthcheck", get(healthcheck))
}

/// Liveness only. It answers as long as the process is serving, which is what
/// a container orchestrator restarts on. It deliberately does not touch the
/// database, so a slow query cannot get the whole service killed.
async fn healthcheck() -> axum::Json<response::Response<Health>> {
    response::ok(Health {
        status: "OK",
        version: env!("CARGO_PKG_VERSION"),
    })
}
