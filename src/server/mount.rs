use axum::Router;

use crate::module::{auth, iam};
use crate::server::Modules;

/// Mounts every module on the root router. New modules are added here, the
/// same way the Go server had a single `Mout` function.
pub fn mount(modules: &Modules) -> Router {
    Router::new()
        .merge(auth::routes(modules.auth.clone()))
        .merge(iam::routes(
            &modules.iam,
            modules.auth.clone(),
            modules.rbac.clone(),
        ))
}
