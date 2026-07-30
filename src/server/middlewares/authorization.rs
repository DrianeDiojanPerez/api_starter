use std::sync::Arc;

use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::Response;

use crate::package::auth::AuthUser;
use crate::package::errdef::Error;
use crate::package::rbac::Engine;

#[derive(Clone)]
pub struct RbacState {
    engine: Arc<dyn Engine>,
    action: &'static str,
}

impl RbacState {
    pub fn new(engine: Arc<dyn Engine>, action: &'static str) -> Self {
        Self { engine, action }
    }
}

/// Runs after [`super::authentication::authenticate`], so a missing identity
/// here means the route was mounted without the auth layer.
pub async fn rbac_guard(
    State(state): State<RbacState>,
    request: Request,
    next: Next,
) -> Result<Response, Error> {
    let user = request
        .extensions()
        .get::<AuthUser>()
        .ok_or_else(|| Error::unauthorized("unauthorized action"))?;

    if !state.engine.can(user.id(), state.action).await {
        return Err(Error::unauthorized("unauthorized action"));
    }

    Ok(next.run(request).await)
}
