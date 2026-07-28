pub mod adapter;
pub mod core;

use std::sync::Arc;

use axum::routing::post;
use axum::Router;

use crate::module::auth::adapter::handler;
use crate::shared::auth::Auth;

/// Public routes. These are the only endpoints reachable without a token.
pub fn routes(auth: Arc<dyn Auth>) -> Router {
    Router::new()
        .route("/v1/login", post(handler::login))
        .route("/v1/refresh-token", post(handler::refresh_token))
        .route("/v1/forgot-password", post(handler::password_recovery))
        .route("/v1/reset-password", post(handler::password_reset))
        .with_state(auth)
}
