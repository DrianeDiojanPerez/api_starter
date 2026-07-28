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

/// Every fallible handler, service and repository returns this type.
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

    /// Wraps an unexpected failure. The cause is logged but never returned to
    /// the caller.
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

/// Central error rendering, mirroring the single `ErrorHandler` the Go server
/// installed on echo so every failure shares one shape.
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
