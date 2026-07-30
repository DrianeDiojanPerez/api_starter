use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Serialize)]
pub struct Error {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct Response<T = Value> {
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pagination: Option<Value>,
    pub error: Option<Error>,
}

impl<T: Serialize> Response<T> {
    pub fn data(data: T) -> Self {
        Self {
            data: Some(data),
            pagination: None,
            error: None,
        }
    }

    pub fn paginated(data: T, pagination: impl Serialize) -> Self {
        Self {
            data: Some(data),
            pagination: serde_json::to_value(pagination).ok(),
            error: None,
        }
    }
}

impl Response<Value> {
    pub fn error(error: Error) -> Self {
        Self {
            data: None,
            pagination: None,
            error: Some(error),
        }
    }
}

pub fn ok<T: Serialize>(data: T) -> axum::Json<Response<T>> {
    axum::Json(Response::data(data))
}

pub fn ok_paginated<T: Serialize>(data: T, pagination: impl Serialize) -> axum::Json<Response<T>> {
    axum::Json(Response::paginated(data, pagination))
}
