use axum::Router;

use crate::module::{auth, iam};
use crate::provider::Provider;

/// Mounts every module on the root router. New modules are added here, the
/// same way the Go server had a single `Mout` function.
pub fn mount(provider: &Provider) -> Router {
    Router::new()
        .merge(auth::routes(provider))
        .merge(iam::routes(provider))
}
