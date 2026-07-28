pub mod adapter;
pub mod core;
mod service;

pub use service::Services;

use axum::routing::{delete, get, patch, post};
use axum::{middleware, Router};

use crate::module::iam::adapter::handler::{permission, user};
use crate::provider::Provider;
use crate::server::middlewares::authorization::{rbac_guard, RbacState};

/// Every route in this module requires an authenticated identity, so the auth
/// middleware is applied once at the module boundary.
pub fn routes(provider: &Provider) -> Router {
    Router::new()
        .merge(user_routes(provider))
        .merge(permission_routes(provider))
        .route_layer(middleware::from_fn_with_state(
            provider.auth.clone(),
            crate::server::middlewares::authentication::authenticate,
        ))
}

fn user_routes(provider: &Provider) -> Router {
    let state = provider.iam.user.clone();

    let index = Router::new()
        .route("/v1/users", get(user::index))
        .route_layer(middleware::from_fn_with_state(
            RbacState::new(provider.rbac.clone(), "Users.View All"),
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

fn permission_routes(provider: &Provider) -> Router {
    Router::new()
        .route("/v1/permissions", get(permission::index))
        .with_state(provider.iam.permission.clone())
}
