use std::collections::BTreeMap;
use std::fmt;

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use crate::shared::response;

/// Domain error codes, kept identical to the Go service so API consumers do
/// not have to change.
pub mod code {
    pub const VALIDATION_FAILED: i32 = 1000;
    pub const NOT_FOUND: i32 = 1001;
    pub const UNAUTHORIZED: i32 = 1002;
    pub const BAD_REQUEST: i32 = 1003;
    pub const RESOURCE_CONFLICT: i32 = 1004;
    pub const UNPROCESSABLE: i32 = 1005;
    pub const FORBIDDEN: i32 = 1006;
    pub const UNKNOWN: i32 = 2000;
}

fn status_for(code: i32) -> StatusCode {
    match code {
        code::NOT_FOUND => StatusCode::NOT_FOUND,
        code::UNAUTHORIZED => StatusCode::UNAUTHORIZED,
        code::BAD_REQUEST => StatusCode::BAD_REQUEST,
        code::RESOURCE_CONFLICT => StatusCode::CONFLICT,
        code::UNPROCESSABLE => StatusCode::UNPROCESSABLE_ENTITY,
        code::FORBIDDEN => StatusCode::FORBIDDEN,
        code::UNKNOWN => StatusCode::INTERNAL_SERVER_ERROR,
        _ => StatusCode::BAD_REQUEST,
    }
}

#[derive(Debug)]
pub struct AppError {
    pub code: i32,
    pub message: String,
    pub cause: Option<String>,
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.cause {
            Some(cause) => write!(f, "code {}: {} (cause: {})", self.code, self.message, cause),
            None => write!(f, "code {}: {}", self.code, self.message),
        }
    }
}

#[derive(Debug, Default)]
pub struct ValidationError {
    pub message: String,
    pub field_violations: BTreeMap<String, Vec<String>>,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "code {}: {}", code::VALIDATION_FAILED, self.message)
    }
}

#[derive(Debug)]
pub enum Error {
    App(AppError),
    Validation(ValidationError),
}

impl Error {
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Error::App(AppError {
            code,
            message: message.into(),
            cause: None,
        })
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(code::NOT_FOUND, message)
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(code::UNAUTHORIZED, message)
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(code::BAD_REQUEST, message)
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(code::RESOURCE_CONFLICT, message)
    }

    pub fn unprocessable(message: impl Into<String>) -> Self {
        Self::new(code::UNPROCESSABLE, message)
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(code::FORBIDDEN, message)
    }

    /// The cause is logged but never returned to the caller.
    pub fn unknown(cause: impl fmt::Display) -> Self {
        Error::App(AppError {
            code: code::UNKNOWN,
            message: "internal server error".to_owned(),
            cause: Some(cause.to_string()),
        })
    }

    pub fn validation(message: impl Into<String>) -> Self {
        Error::Validation(ValidationError {
            message: message.into(),
            field_violations: BTreeMap::new(),
        })
    }

    pub fn with_cause(mut self, cause: impl fmt::Display) -> Self {
        if let Error::App(err) = &mut self {
            err.cause = Some(cause.to_string());
        }
        self
    }

    pub fn add_violation(mut self, field: impl Into<String>, message: impl Into<String>) -> Self {
        self.push_violation(field, message);
        self
    }

    pub fn push_violation(&mut self, field: impl Into<String>, message: impl Into<String>) {
        if let Error::Validation(err) = self {
            err.field_violations
                .entry(field.into())
                .or_default()
                .push(message.into());
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::App(err) => err.fmt(f),
            Error::Validation(err) => err.fmt(f),
        }
    }
}

impl std::error::Error for Error {}

impl From<sqlx::Error> for Error {
    fn from(err: sqlx::Error) -> Self {
        Error::unknown(err)
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        tracing::info!(error = %self, "Error Handler Catch");

        let (status, body) = match self {
            Error::Validation(err) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                response::Error {
                    code: StatusCode::UNPROCESSABLE_ENTITY.as_u16() as i32,
                    message: err.message,
                    errors: Some(
                        serde_json::to_value(err.field_violations)
                            .unwrap_or(serde_json::Value::Null),
                    ),
                },
            ),
            Error::App(err) => {
                let status = status_for(err.code);
                (
                    status,
                    response::Error {
                        code: status.as_u16() as i32,
                        message: err.message,
                        errors: None,
                    },
                )
            }
        };

        (status, axum::Json(response::Response::error(body))).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use axum::body::to_bytes;
    use serde_json::Value;

    async fn render(error: Error) -> (StatusCode, Value) {
        let response = error.into_response();
        let status = response.status();

        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("the body should be readable");

        (
            status,
            serde_json::from_slice(&bytes).expect("the body should be json"),
        )
    }

    #[test]
    fn maps_every_domain_code_to_a_status() {
        assert_eq!(status_for(code::NOT_FOUND), StatusCode::NOT_FOUND);
        assert_eq!(status_for(code::UNAUTHORIZED), StatusCode::UNAUTHORIZED);
        assert_eq!(status_for(code::BAD_REQUEST), StatusCode::BAD_REQUEST);
        assert_eq!(status_for(code::RESOURCE_CONFLICT), StatusCode::CONFLICT);
        assert_eq!(
            status_for(code::UNPROCESSABLE),
            StatusCode::UNPROCESSABLE_ENTITY
        );
        assert_eq!(status_for(code::FORBIDDEN), StatusCode::FORBIDDEN);
        assert_eq!(status_for(code::UNKNOWN), StatusCode::INTERNAL_SERVER_ERROR);
        // Anything unmapped falls back to a bad request rather than a 500.
        assert_eq!(status_for(4242), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn renders_an_app_error_in_the_shared_envelope() {
        let (status, body) = render(Error::not_found("user does not exists")).await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["data"], Value::Null);
        assert_eq!(body["error"]["code"], 404);
        assert_eq!(body["error"]["message"], "user does not exists");
        assert!(body["error"].get("errors").is_none());
    }

    #[tokio::test]
    async fn renders_field_violations_on_a_validation_error() {
        let error = Error::validation("failed payload validation")
            .add_violation("email", "field must be of email format")
            .add_violation("password", "field is required and cannot be empty")
            .add_violation("password", "field is too short");

        let (status, body) = render(error).await;

        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["error"]["code"], 422);
        assert_eq!(
            body["error"]["errors"]["email"][0],
            "field must be of email format"
        );
        assert_eq!(
            body["error"]["errors"]["password"].as_array().map(Vec::len),
            Some(2),
            "violations on the same field accumulate"
        );
    }

    #[tokio::test]
    async fn never_leaks_the_cause_of_an_unknown_error() {
        let (status, body) = render(Error::unknown("connection refused to 10.0.0.1:5432")).await;

        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body["error"]["message"], "internal server error");
        assert!(!body.to_string().contains("10.0.0.1"));
    }

    #[test]
    fn keeps_the_cause_on_the_log_representation() {
        let error = Error::unauthorized("invalid token").with_cause("signature mismatch");

        assert_eq!(
            error.to_string(),
            "code 1002: invalid token (cause: signature mismatch)"
        );
    }

    #[test]
    fn a_database_failure_converts_to_an_unknown_error() {
        let error: Error = sqlx::Error::RowNotFound.into();

        match error {
            Error::App(err) => assert_eq!(err.code, code::UNKNOWN),
            other => panic!("expected an app error, got {other:?}"),
        }
    }

    #[test]
    fn adding_a_violation_to_an_app_error_is_a_no_op() {
        let error = Error::bad_request("nope").add_violation("field", "message");

        match error {
            Error::App(err) => assert_eq!(err.message, "nope"),
            other => panic!("expected an app error, got {other:?}"),
        }
    }
}
