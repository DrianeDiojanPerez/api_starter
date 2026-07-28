pub mod adapter;
pub mod core;

use axum::routing::post;
use axum::Router;

use crate::module::auth::adapter::handler;
use crate::provider::Provider;

/// Public routes. These are the only endpoints reachable without a token.
pub fn routes(provider: &Provider) -> Router {
    Router::new()
        .route("/v1/login", post(handler::login))
        .route("/v1/refresh-token", post(handler::refresh_token))
        .route("/v1/forgot-password", post(handler::password_recovery))
        .route("/v1/reset-password", post(handler::password_reset))
        .with_state(provider.auth.clone())
}
