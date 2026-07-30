use axum::Router;

use crate::module::{auth, iam};
use crate::server::Modules;

pub fn mount(modules: &Modules) -> Router {
    Router::new()
        .merge(auth::routes(modules.auth.clone()))
        .merge(iam::routes(
            &modules.iam,
            modules.auth.clone(),
            modules.rbac.clone(),
        ))
}
