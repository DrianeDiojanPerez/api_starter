//! End to end tests for the protected IAM routes: the auth middleware, the
//! RBAC guard, the extractors and the response shapes.

mod support;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::{json, Value};
use uuid::Uuid;

use support::{authorized, TestApp, VALID_TOKEN};

// ──── Authentication ──────────────────────────────────

#[tokio::test]
async fn a_protected_route_rejects_a_request_without_a_token() {
    let app = TestApp::new();

    let (status, body) = app.get("/v1/users/my-user").await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"]["message"], "malformed or missing jwt token");
}

#[tokio::test]
async fn a_protected_route_rejects_a_non_bearer_scheme() {
    let app = TestApp::new();

    let (status, _) = app
        .send(
            Request::builder()
                .uri("/v1/users/my-user")
                .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
                .body(Body::empty())
                .expect("the request should build"),
        )
        .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_protected_route_rejects_an_empty_bearer_token() {
    let app = TestApp::new();

    let (status, _) = app
        .send(authorized("GET", "/v1/users/my-user", "", None))
        .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_protected_route_rejects_an_unknown_token() {
    let app = TestApp::new();

    let (status, _) = app
        .send(authorized("GET", "/v1/users/my-user", "forged", None))
        .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_valid_token_resolves_the_identity_for_the_handler() {
    let app = TestApp::new();

    let (status, body) = app.get_authorized("/v1/users/my-user").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"]["user_id"], app.user.id.to_string());
    assert_eq!(body["data"]["user_name"], "admin");
}

// ──── Authorization ──────────────────────────────────

#[tokio::test]
async fn the_user_index_requires_the_view_all_permission() {
    let app = TestApp::with_permissions(&[]);

    let (status, body) = app.get_authorized("/v1/users").await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"]["message"], "unauthorized action");
}

#[tokio::test]
async fn the_user_index_is_allowed_with_the_permission() {
    let app = TestApp::with_permissions(&["Users.View All"]);

    let (status, _) = app.get_authorized("/v1/users").await;

    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn the_guard_checks_the_action_the_route_declares() {
    let app = TestApp::with_permissions(&["Users.View All"]);

    app.get_authorized("/v1/users").await;

    let checks = app.calls.permission_checks.lock().unwrap();
    let (user_id, action) = checks.first().expect("the guard should run");

    assert_eq!(*user_id, app.user.id);
    assert_eq!(action, "Users.View All");
}

#[tokio::test]
async fn the_guard_runs_after_authentication_not_before() {
    let app = TestApp::with_permissions(&["Users.View All"]);

    // No token at all, so the request must not even reach the RBAC engine.
    let (status, _) = app.get("/v1/users").await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(app.calls.permission_checks.lock().unwrap().is_empty());
}

#[tokio::test]
async fn the_other_user_routes_are_not_behind_the_view_all_permission() {
    let app = TestApp::with_permissions(&[]);

    let (status, _) = app.get_authorized("/v1/users/my-user").await;

    assert_eq!(status, StatusCode::OK);
    assert!(app.calls.permission_checks.lock().unwrap().is_empty());
}

// ──── Users ──────────────────────────────────

#[tokio::test]
async fn the_user_index_returns_the_pagination_envelope() {
    let app = TestApp::new();

    let (status, body) = app.get_authorized("/v1/users?page=1&per_page=5").await;

    assert_eq!(status, StatusCode::OK);
    assert!(body["data"].is_array());
    assert_eq!(body["pagination"]["current_page"], 1);
    assert_eq!(body["pagination"]["per_page"], 5);
    assert_eq!(body["pagination"]["total_count"], 1);
}

#[tokio::test]
async fn the_user_index_forwards_the_query_string_as_filters() {
    let app = TestApp::new();

    app.get_authorized(
        "/v1/users?page=2&per_page=25&sort_by=email&order=asc&status=Active&role=Admin",
    )
    .await;

    let requests = app.calls.list_requests.lock().unwrap();
    let request = requests.first().expect("the service should be called");

    assert_eq!(request.page, 2);
    assert_eq!(request.per_page, 25);
    assert_eq!(request.sort_by, "email");
    assert_eq!(request.order, "asc");
    assert_eq!(request.filter("status"), Some("Active"));
    assert_eq!(request.filter("role"), Some("Admin"));
}

#[tokio::test]
async fn out_of_range_pagination_is_clamped_before_the_service_sees_it() {
    let app = TestApp::new();

    app.get_authorized("/v1/users?page=-3&per_page=9999&order=sideways")
        .await;

    let requests = app.calls.list_requests.lock().unwrap();
    let request = requests.first().expect("the service should be called");

    assert_eq!(request.page, 1);
    assert_eq!(request.per_page, 10);
    assert!(request.order.is_empty());
}

#[tokio::test]
async fn getting_a_user_by_id_returns_the_flattened_shape() {
    let app = TestApp::new();

    let (status, body) = app
        .get_authorized(&format!("/v1/users/{}", app.user.id))
        .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["data"]["department"]["department_name"],
        "Administration"
    );
    assert_eq!(
        body["data"]["department"]["company"]["name"],
        "Example Company Ltd"
    );
    assert_eq!(body["data"]["roles"][0], "Staff");
}

#[tokio::test]
async fn getting_a_missing_user_is_a_not_found() {
    let app = TestApp::new();

    let (status, body) = app
        .get_authorized(&format!("/v1/users/{}", Uuid::new_v4()))
        .await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["message"], "user does not exists");
}

#[tokio::test]
async fn a_path_that_is_not_a_uuid_is_rejected() {
    let app = TestApp::new();

    let (status, _) = app.get_authorized("/v1/users/not-a-uuid").await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn the_static_my_user_route_wins_over_the_uuid_route() {
    let app = TestApp::new();

    let (status, body) = app.get_authorized("/v1/users/my-user").await;

    // If the dynamic route matched first, this would be a 400 for a bad uuid.
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"]["user_id"], app.user.id.to_string());
}

#[tokio::test]
async fn creating_a_user_forwards_the_payload_and_forces_the_active_status() {
    let app = TestApp::new();

    let (status, body) = app
        .authorized(
            "POST",
            "/v1/users",
            Some(json!({
                "user_name": "jsmith",
                "email": "jsmith@example.com",
                "password": "Sup3r$ecret",
                "first_name": "John",
                "last_name": "Smith",
                "department": 1,
                "roles": ["Staff"]
            })),
        )
        .await;

    assert_eq!(status, StatusCode::OK);
    assert!(body["data"].is_string());

    let created = app.calls.created_users.lock().unwrap();
    let user = created.first().expect("the service should be called");

    assert_eq!(user.user_name, "jsmith");
    assert_eq!(user.department_id, 1);
    assert_eq!(user.roles, vec!["Staff".to_owned()]);
    assert_eq!(user.status, 1, "a new user is always created active");
    assert_eq!(
        user.password, "Sup3r$ecret",
        "hashing belongs to the service, not the handler"
    );
}

#[tokio::test]
async fn creating_a_user_defaults_the_optional_fields() {
    let app = TestApp::new();

    let (status, _) = app
        .authorized(
            "POST",
            "/v1/users",
            Some(json!({
                "user_name": "jsmith",
                "email": "jsmith@example.com",
                "password": "Sup3r$ecret",
                "first_name": "John",
                "last_name": "Smith",
                "department": 1
            })),
        )
        .await;

    assert_eq!(status, StatusCode::OK);

    let created = app.calls.created_users.lock().unwrap();
    let user = created.first().expect("the service should be called");

    assert_eq!(user.avatar_id, "");
    assert!(user.roles.is_empty());
}

#[tokio::test]
async fn creating_a_user_validates_every_field() {
    let app = TestApp::new();

    let (status, body) = app
        .authorized(
            "POST",
            "/v1/users",
            Some(json!({
                "user_name": "",
                "email": "not-an-email",
                "password": "weak",
                "first_name": "",
                "last_name": "",
                "department": 0
            })),
        )
        .await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    let errors = &body["error"]["errors"];
    for field in [
        "user_name",
        "email",
        "password",
        "first_name",
        "last_name",
        "department",
    ] {
        assert!(errors[field].is_array(), "{field} should be reported");
    }

    assert!(app.calls.created_users.lock().unwrap().is_empty());
}

#[tokio::test]
async fn patching_a_user_forwards_every_allowed_field() {
    let app = TestApp::new();

    let (status, body) = app
        .authorized(
            "PATCH",
            &format!("/v1/users/{}", app.user.id),
            Some(json!({
                "email": "new@example.com",
                "status": "Deleted",
                "add_roles": ["Developer"],
                "remove_roles": ["Staff"]
            })),
        )
        .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"], "successfully update user");

    let updates = app.calls.updates.lock().unwrap();
    let (user_id, fields) = updates.first().expect("the service should be called");

    assert_eq!(*user_id, app.user.id);
    assert_eq!(fields.email.as_deref(), Some("new@example.com"));
    assert_eq!(fields.status.as_deref(), Some("Deleted"));
    assert_eq!(
        fields.add_roles.as_deref(),
        Some(["Developer".to_owned()].as_slice())
    );
    assert_eq!(
        fields.remove_roles.as_deref(),
        Some(["Staff".to_owned()].as_slice())
    );
}

#[tokio::test]
async fn patching_a_user_rejects_an_unknown_field() {
    let app = TestApp::new();

    let (status, body) = app
        .authorized(
            "PATCH",
            &format!("/v1/users/{}", app.user.id),
            Some(json!({ "is_superuser": true })),
        )
        .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["message"], "invalid json body");
    assert!(app.calls.updates.lock().unwrap().is_empty());
}

#[tokio::test]
async fn patching_my_user_drops_the_privileged_fields() {
    let app = TestApp::new();

    let (status, _) = app
        .authorized(
            "PATCH",
            "/v1/users/my-user",
            Some(json!({
                "first_name": "Johnny",
                "last_name": "Smith",
                "avatar_id": "avatar.gif",
                "department_id": 2,
                "status": "Deleted",
                "password": "Sup3r$ecret",
                "email": "escalation@example.com",
                "add_roles": ["Admin"],
                "remove_roles": ["Staff"]
            })),
        )
        .await;

    assert_eq!(status, StatusCode::OK);

    let updates = app.calls.updates.lock().unwrap();
    let (user_id, fields) = updates.first().expect("the service should be called");

    assert_eq!(*user_id, app.user.id, "always the caller, never a path id");
    assert_eq!(fields.first_name.as_deref(), Some("Johnny"));
    assert_eq!(fields.last_name.as_deref(), Some("Smith"));
    assert_eq!(fields.avatar_id.as_deref(), Some("avatar.gif"));
    assert_eq!(fields.department_id, Some(2));

    assert!(
        fields.status.is_none(),
        "a user cannot change their own status"
    );
    assert!(
        fields.password.is_none(),
        "password changes go through the reset flow"
    );
    assert!(
        fields.email.is_none(),
        "a user cannot change their own email"
    );
    assert!(
        fields.add_roles.is_none(),
        "a user cannot grant themselves a role"
    );
    assert!(fields.remove_roles.is_none());
}

#[tokio::test]
async fn an_empty_patch_body_is_accepted_as_a_no_op() {
    let app = TestApp::new();

    let (status, _) = app
        .authorized(
            "PATCH",
            &format!("/v1/users/{}", app.user.id),
            Some(json!({})),
        )
        .await;

    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn deleting_a_user_echoes_the_id_back() {
    let app = TestApp::new();

    let (status, body) = app
        .authorized("DELETE", &format!("/v1/users/{}", app.user.id), None)
        .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"]["message"], "User deleted successfully");
    assert_eq!(body["data"]["user_id"], app.user.id.to_string());

    let deleted = app.calls.deleted.lock().unwrap();
    assert_eq!(deleted.first(), Some(&app.user.id));
}

#[tokio::test]
async fn a_method_the_route_does_not_support_is_not_a_404() {
    let app = TestApp::new();

    let (status, _) = app.authorized("PUT", "/v1/users", None).await;

    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
}

// ──── Permissions ──────────────────────────────────

#[tokio::test]
async fn listing_permissions_requires_a_token() {
    let app = TestApp::new();

    let (status, _) = app.get("/v1/permissions").await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn listing_permissions_returns_the_resource_and_module() {
    let app = TestApp::with_permissions(&[]);

    let (status, body) = app.get_authorized("/v1/permissions").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"][0]["name"], "View All");
    assert_eq!(body["data"][0]["resource"], "Users");
    assert_eq!(body["data"][0]["module"], "IAM Module");
    assert_eq!(body["error"], Value::Null);
}

#[tokio::test]
async fn the_permission_id_stays_internal() {
    let app = TestApp::new();

    let (_, body) = app.get_authorized("/v1/permissions").await;

    assert!(body["data"][0].get("id").is_none());
}

#[tokio::test]
async fn the_token_is_read_from_the_authorization_header_only() {
    let app = TestApp::new();

    let (status, _) = app
        .send(
            Request::builder()
                .uri(format!("/v1/users/my-user?token={VALID_TOKEN}"))
                .body(Body::empty())
                .expect("the request should build"),
        )
        .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
