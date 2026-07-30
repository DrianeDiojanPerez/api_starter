use std::sync::Arc;

use axum::extract::{FromRequestParts, Request, State};
use axum::http::request::Parts;
use axum::middleware::Next;
use axum::response::Response;

use crate::shared::auth::{Auth, AuthUser};
use crate::shared::errdef::Error;

const BEARER: &str = "Bearer ";

pub async fn authenticate(
    State(auth): State<Arc<dyn Auth>>,
    mut request: Request,
    next: Next,
) -> Result<Response, Error> {
    let token = request
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix(BEARER))
        .filter(|token| !token.is_empty())
        .ok_or_else(|| Error::unauthorized("malformed or missing jwt token"))?
        .to_owned();

    let user = auth
        .get_identity(&token)
        .await
        .map_err(|err| Error::unauthorized("malformed or missing jwt token").with_cause(err))?;

    request.extensions_mut().insert(AuthUser(user));

    Ok(next.run(request).await)
}

impl<S: Send + Sync> FromRequestParts<S> for AuthUser {
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<AuthUser>()
            .cloned()
            .ok_or_else(|| Error::unauthorized("user not found in context"))
    }
}
