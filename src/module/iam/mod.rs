pub mod adapter;
pub mod core;
mod service;

pub use service::Services;

use std::sync::Arc;

use axum::routing::{delete, get, patch, post};
use axum::{middleware, Router};

use crate::module::iam::adapter::handler::{permission, user};
use crate::server::middlewares::authentication::authenticate;
use crate::server::middlewares::authorization::{rbac_guard, RbacState};
use crate::shared::auth::Auth;
use crate::shared::rbac::Engine;

/// Every route in this module requires an authenticated identity, so the auth
/// middleware is applied once at the module boundary.
pub fn routes(services: &Services, auth: Arc<dyn Auth>, rbac: Arc<dyn Engine>) -> Router {
    Router::new()
        .merge(user_routes(services, rbac))
        .merge(permission_routes(services))
        .route_layer(middleware::from_fn_with_state(auth, authenticate))
}

fn user_routes(services: &Services, rbac: Arc<dyn Engine>) -> Router {
    let state = services.user.clone();

    let index = Router::new()
        .route("/v1/users", get(user::index))
        .route_layer(middleware::from_fn_with_state(
            RbacState::new(rbac, "Users.View All"),
            rbac_guard,
        ))
        .with_state(state.clone());

    Router::new()
        .route("/v1/users", post(user::create))
        .route("/v1/users/my-user", get(user::get_my_user))
        .route("/v1/users/my-user", patch(user::patch_my_user))
        .route("/v1/users/{user-id}", get(user::get))
        .route("/v1/users/{user-id}", patch(user::patch))
        .route("/v1/users/{user-id}", delete(user::delete))
        .with_state(state)
        .merge(index)
}

fn permission_routes(services: &Services) -> Router {
    Router::new()
        .route("/v1/permissions", get(permission::index))
        .with_state(services.permission.clone())
}
