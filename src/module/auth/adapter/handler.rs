use std::sync::Arc;

use axum::extract::State;
use axum::Json as AxumJson;
use serde::Deserialize;
use validator::Validate;

use crate::module::auth::core::domain::LoginResponse;
use crate::package::auth::Auth;
use crate::package::errdef::Error;
use crate::package::extract::ValidatedJson;
use crate::package::response::{self, Response};
use crate::package::validation::strong_password;

pub type AuthState = Arc<dyn Auth>;

#[derive(Debug, Deserialize, Validate)]
pub struct LoginRequest {
    #[validate(length(min = 1, message = "field is required and cannot be empty"))]
    pub email: String,
    #[validate(length(min = 1, message = "field is required and cannot be empty"))]
    pub password: String,
}

#[tracing::instrument(name = "AuthenticationService.Login", skip_all)]
pub async fn login(
    State(service): State<AuthState>,
    ValidatedJson(payload): ValidatedJson<LoginRequest>,
) -> Result<AxumJson<Response<LoginResponse>>, Error> {
    let tokens = service
        .generate_token(&payload.email, &payload.password)
        .await?;

    Ok(response::ok(LoginResponse::from(tokens)))
}

#[derive(Debug, Deserialize, Validate)]
pub struct RefreshRequest {
    #[validate(length(min = 1, message = "field is required and cannot be empty"))]
    pub token: String,
}

#[tracing::instrument(name = "AuthenticationService.RefreshToken", skip_all)]
pub async fn refresh_token(
    State(service): State<AuthState>,
    ValidatedJson(payload): ValidatedJson<RefreshRequest>,
) -> Result<AxumJson<Response<LoginResponse>>, Error> {
    let tokens = service.refresh_token(&payload.token).await?;

    Ok(response::ok(LoginResponse::from(tokens)))
}

#[derive(Debug, Deserialize, Validate)]
pub struct PasswordRecoveryRequest {
    #[validate(email)]
    pub email: String,
    #[validate(url)]
    pub callback_uri: String,
}

#[tracing::instrument(name = "AuthenticationService.PasswordRecovery", skip_all)]
pub async fn password_recovery(
    State(service): State<AuthState>,
    ValidatedJson(payload): ValidatedJson<PasswordRecoveryRequest>,
) -> Result<AxumJson<Response<&'static str>>, Error> {
    service
        .password_recovery(&payload.email, &payload.callback_uri)
        .await?;

    Ok(response::ok("password recovery email sent"))
}

#[derive(Debug, Deserialize, Validate)]
pub struct PasswordResetRequest {
    #[validate(length(min = 1, message = "field is required and cannot be empty"))]
    pub token: String,
    #[validate(custom(function = "strong_password"))]
    pub password: String,
}

#[tracing::instrument(name = "AuthenticationService.PasswordReset", skip_all)]
pub async fn password_reset(
    State(service): State<AuthState>,
    ValidatedJson(payload): ValidatedJson<PasswordResetRequest>,
) -> Result<AxumJson<Response<&'static str>>, Error> {
    service
        .reset_password(&payload.token, &payload.password)
        .await?;

    Ok(response::ok("password has been changed"))
}
