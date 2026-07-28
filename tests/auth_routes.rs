//! End to end tests for the public auth routes, driving the real router.

mod support;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;

use support::{TestApp, VALID_TOKEN};

#[tokio::test]
async fn login_returns_the_token_pair_in_the_shared_envelope() {
    let app = TestApp::new();

    let (status, body) = app
        .post(
            "/v1/login",
            json!({ "email": "admin@example.com", "password": "password" }),
        )
        .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"]["token"], VALID_TOKEN);
    assert_eq!(body["data"]["refresh_token"], "a-valid-refresh-token");
    assert_eq!(body["error"], serde_json::Value::Null);
}

#[tokio::test]
async fn login_rejects_bad_credentials_without_saying_which_part_was_wrong() {
    let app = TestApp::new();

    let (status, body) = app
        .post(
            "/v1/login",
            json!({ "email": "admin@example.com", "password": "wrong" }),
        )
        .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"]["message"], "invalid username or password");
    assert_eq!(body["data"], serde_json::Value::Null);
}

#[tokio::test]
async fn login_reports_missing_fields_as_field_violations() {
    let app = TestApp::new();

    let (status, body) = app
        .post("/v1/login", json!({ "email": "", "password": "" }))
        .await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"]["code"], 422);
    assert!(body["error"]["errors"]["email"].is_array());
    assert!(body["error"]["errors"]["password"].is_array());
}

#[tokio::test]
async fn a_malformed_body_is_a_bad_request_not_a_crash() {
    let app = TestApp::new();

    let (status, body) = app
        .send(
            Request::builder()
                .method("POST")
                .uri("/v1/login")
                .header("content-type", "application/json")
                .body(Body::from("{not json"))
                .expect("the request should build"),
        )
        .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["message"], "invalid json body");
}

#[tokio::test]
async fn a_missing_content_type_is_a_bad_request() {
    let app = TestApp::new();

    let (status, body) = app
        .send(
            Request::builder()
                .method("POST")
                .uri("/v1/login")
                .body(Body::from(r#"{"email":"a@b.com","password":"x"}"#))
                .expect("the request should build"),
        )
        .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["message"], "invalid json body");
}

#[tokio::test]
async fn refresh_exchanges_a_valid_refresh_token() {
    let app = TestApp::new();

    let (status, body) = app
        .post(
            "/v1/refresh-token",
            json!({ "token": "a-valid-refresh-token" }),
        )
        .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"]["token"], VALID_TOKEN);
}

#[tokio::test]
async fn refresh_rejects_an_unknown_token() {
    let app = TestApp::new();

    let (status, body) = app
        .post("/v1/refresh-token", json!({ "token": "nope" }))
        .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        body["error"]["message"],
        "invalid or malformed refresh token"
    );
}

#[tokio::test]
async fn forgot_password_accepts_a_known_address() {
    let app = TestApp::new();

    let (status, body) = app
        .post(
            "/v1/forgot-password",
            json!({
                "email": "admin@example.com",
                "callback_uri": "https://example.com/reset?token="
            }),
        )
        .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"], "password recovery email sent");
}

#[tokio::test]
async fn forgot_password_reports_an_unknown_address() {
    let app = TestApp::new();

    let (status, _) = app
        .post(
            "/v1/forgot-password",
            json!({
                "email": "nobody@example.com",
                "callback_uri": "https://example.com/reset?token="
            }),
        )
        .await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn forgot_password_validates_the_callback_uri() {
    let app = TestApp::new();

    let (status, body) = app
        .post(
            "/v1/forgot-password",
            json!({ "email": "not-an-email", "callback_uri": "not a uri" }),
        )
        .await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        body["error"]["errors"]["email"][0],
        "field must be of email format"
    );
    assert_eq!(
        body["error"]["errors"]["callback_uri"][0],
        "filed must be of URI format"
    );
}

#[tokio::test]
async fn reset_password_consumes_a_valid_token() {
    let app = TestApp::new();

    let (status, body) = app
        .post(
            "/v1/reset-password",
            json!({ "token": "a-valid-reset-token", "password": "N3wP@ssword" }),
        )
        .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"], "password has been changed");
}

#[tokio::test]
async fn reset_password_enforces_the_strong_password_rule() {
    let app = TestApp::new();

    let (status, body) = app
        .post(
            "/v1/reset-password",
            json!({ "token": "a-valid-reset-token", "password": "weak" }),
        )
        .await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(body["error"]["errors"]["password"][0]
        .as_str()
        .expect("the violation should be a string")
        .contains("at least 8 characters"));
}

#[tokio::test]
async fn reset_password_rejects_an_expired_token() {
    let app = TestApp::new();

    let (status, body) = app
        .post(
            "/v1/reset-password",
            json!({ "token": "stale", "password": "N3wP@ssword" }),
        )
        .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["message"], "invalid or expired token");
}

#[tokio::test]
async fn the_auth_routes_need_no_token() {
    let app = TestApp::new();

    // No `Authorization` header at all, and this is still not a 401.
    let (status, _) = app
        .post(
            "/v1/login",
            json!({ "email": "admin@example.com", "password": "password" }),
        )
        .await;

    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn an_unknown_route_uses_the_shared_error_envelope() {
    let app = TestApp::new();

    let (status, body) = app.get("/v1/does-not-exist").await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], 404);
    assert_eq!(body["error"]["message"], "route not found");
}

#[tokio::test]
async fn every_response_carries_a_request_id() {
    let app = TestApp::new();

    let response = tower::ServiceExt::oneshot(
        app.router.clone(),
        Request::builder()
            .uri("/v1/does-not-exist")
            .body(Body::empty())
            .expect("the request should build"),
    )
    .await
    .expect("the router should respond");

    assert!(response.headers().contains_key("x-request-id"));
}
