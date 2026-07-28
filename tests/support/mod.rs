//! Test doubles and helpers shared by the HTTP tests.
//!
//! These drive the real router, the real middleware stack and the real
//! extractors, with the services replaced so no database is involved.

// Every test binary compiles this module, so helpers only one of them uses
// would otherwise be reported as dead code.
#![allow(dead_code)]

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use axum::body::Body;
use axum::http::{Request, Response, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;
use uuid::Uuid;

use api_starter::module::iam;
use api_starter::module::iam::core::domain::{
    Company, CreateUser, Department, Permission, Role, Status, User,
};
use api_starter::module::iam::core::ports::{PermissionService, UpdateUser, UserService};
use api_starter::sdk::{AuthenticationTokens, User as SdkUser};
use api_starter::server::{self, Modules};
use api_starter::shared::auth::Auth;
use api_starter::shared::errdef::Error;
use api_starter::shared::pagination::{Data, ListRequest};
use api_starter::shared::rbac::Engine;

pub const VALID_TOKEN: &str = "a-valid-token";

/// Records everything the fakes were asked to do, so a test can assert on the
/// values that reached the service layer.
#[derive(Default)]
pub struct Calls {
    pub list_requests: Mutex<Vec<ListRequest>>,
    pub created_users: Mutex<Vec<CreateUser>>,
    pub updates: Mutex<Vec<(Uuid, UpdateUser)>>,
    pub deleted: Mutex<Vec<Uuid>>,
    pub permission_checks: Mutex<Vec<(Uuid, String)>>,
}

pub struct FakeAuth {
    pub user: SdkUser,
}

#[async_trait]
impl Auth for FakeAuth {
    async fn generate_token(
        &self,
        email: &str,
        password: &str,
    ) -> Result<AuthenticationTokens, Error> {
        if email == self.user.email && password == "password" {
            return Ok(AuthenticationTokens {
                token: VALID_TOKEN.to_owned(),
                refresh_token: "a-valid-refresh-token".to_owned(),
            });
        }

        Err(Error::unauthorized("invalid username or password"))
    }

    async fn refresh_token(&self, refresh_token: &str) -> Result<AuthenticationTokens, Error> {
        if refresh_token == "a-valid-refresh-token" {
            return Ok(AuthenticationTokens {
                token: VALID_TOKEN.to_owned(),
                refresh_token: "a-valid-refresh-token".to_owned(),
            });
        }

        Err(Error::unauthorized("invalid or malformed refresh token"))
    }

    async fn get_identity(&self, access_token: &str) -> Result<SdkUser, Error> {
        if access_token == VALID_TOKEN {
            return Ok(self.user.clone());
        }

        Err(Error::unauthorized("invalid or malformed refresh token"))
    }

    async fn password_recovery(&self, email: &str, _callback_uri: &str) -> Result<(), Error> {
        if email == self.user.email {
            return Ok(());
        }

        Err(Error::not_found("invalid email address"))
    }

    async fn reset_password(&self, token: &str, _new_password: &str) -> Result<(), Error> {
        if token == "a-valid-reset-token" {
            return Ok(());
        }

        Err(Error::bad_request("invalid or expired token"))
    }
}

pub struct FakeRbac {
    pub allowed: Vec<String>,
    pub calls: Arc<Calls>,
}

#[async_trait]
impl Engine for FakeRbac {
    async fn can(&self, user_id: Uuid, action: &str) -> bool {
        self.calls
            .permission_checks
            .lock()
            .unwrap()
            .push((user_id, action.to_owned()));

        self.allowed.iter().any(|allowed| allowed == action)
    }

    async fn can_any(&self, user_id: Uuid, actions: &[&str]) -> bool {
        for action in actions {
            if self.can(user_id, action).await {
                return true;
            }
        }
        false
    }
}

pub struct FakeUserService {
    pub users: Vec<User>,
    pub calls: Arc<Calls>,
}

#[async_trait]
impl UserService for FakeUserService {
    async fn index(&self, request: ListRequest) -> Result<Data<User>, Error> {
        let page = request.page;
        let per_page = request.per_page;
        self.calls.list_requests.lock().unwrap().push(request);

        Ok(Data::new(
            self.users.clone(),
            self.users.len() as i64,
            page,
            per_page,
        ))
    }

    async fn create(&self, new_user: CreateUser) -> Result<Uuid, Error> {
        let id = Uuid::new_v4();
        self.calls.created_users.lock().unwrap().push(new_user);
        Ok(id)
    }

    async fn find_by_id(&self, user_id: Uuid) -> Result<User, Error> {
        self.users
            .iter()
            .find(|user| user.id == user_id)
            .cloned()
            .ok_or_else(|| Error::not_found("user does not exists"))
    }

    async fn partial_update(&self, user_id: Uuid, fields: UpdateUser) -> Result<(), Error> {
        self.calls.updates.lock().unwrap().push((user_id, fields));
        Ok(())
    }

    async fn delete(&self, user_id: Uuid) -> Result<(), Error> {
        self.calls.deleted.lock().unwrap().push(user_id);
        Ok(())
    }
}

pub struct FakePermissionService {
    pub permissions: Vec<Permission>,
}

#[async_trait]
impl PermissionService for FakePermissionService {
    async fn list_all(&self) -> Result<Vec<Permission>, Error> {
        Ok(self.permissions.clone())
    }
}

/// A fully wired app with fakes behind it, plus the recorder the test asserts
/// on and the identity the valid token resolves to.
pub struct TestApp {
    pub router: Router,
    pub calls: Arc<Calls>,
    pub user: SdkUser,
}

impl TestApp {
    /// `allowed` lists the RBAC actions the authenticated user holds.
    pub fn with_permissions(allowed: &[&str]) -> Self {
        let user = a_domain_user();
        let identity = SdkUser {
            id: user.id,
            email: user.email.clone(),
            user_name: user.user_name.clone(),
            password: String::new(),
            roles: vec!["Staff".to_owned()],
        };

        let calls = Arc::new(Calls::default());

        let modules = Modules {
            auth: Arc::new(FakeAuth {
                user: identity.clone(),
            }),
            rbac: Arc::new(FakeRbac {
                allowed: allowed.iter().map(|a| (*a).to_owned()).collect(),
                calls: calls.clone(),
            }),
            iam: iam::Services {
                user: Arc::new(FakeUserService {
                    users: vec![user],
                    calls: calls.clone(),
                }),
                permission: Arc::new(FakePermissionService {
                    permissions: vec![Permission {
                        id: 1,
                        name: "View All".to_owned(),
                        resource: "Users".to_owned(),
                        module: "IAM Module".to_owned(),
                    }],
                }),
            },
        };

        Self {
            router: server::router(&modules),
            calls,
            user: identity,
        }
    }

    pub fn new() -> Self {
        Self::with_permissions(&["Users.View All"])
    }

    pub async fn send(&self, request: Request<Body>) -> (StatusCode, Value) {
        let response: Response<Body> = self
            .router
            .clone()
            .oneshot(request)
            .await
            .expect("the router should always respond");

        let status = response.status();
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("the body should be readable")
            .to_bytes();

        let body = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap_or(Value::Null)
        };

        (status, body)
    }

    pub async fn get(&self, uri: &str) -> (StatusCode, Value) {
        self.send(
            Request::builder()
                .uri(uri)
                .body(Body::empty())
                .expect("the request should build"),
        )
        .await
    }

    pub async fn get_authorized(&self, uri: &str) -> (StatusCode, Value) {
        self.send(authorized("GET", uri, VALID_TOKEN, None)).await
    }

    pub async fn post(&self, uri: &str, body: Value) -> (StatusCode, Value) {
        self.send(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .expect("the request should build"),
        )
        .await
    }

    pub async fn authorized(
        &self,
        method: &str,
        uri: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        self.send(authorized(method, uri, VALID_TOKEN, body)).await
    }
}

pub fn authorized(method: &str, uri: &str, token: &str, body: Option<Value>) -> Request<Body> {
    let builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("authorization", format!("Bearer {token}"));

    match body {
        Some(body) => builder
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .expect("the request should build"),
        None => builder
            .body(Body::empty())
            .expect("the request should build"),
    }
}

pub fn a_domain_user() -> User {
    User {
        id: Uuid::new_v4(),
        user_name: "admin".to_owned(),
        avatar_id: String::new(),
        email: "admin@example.com".to_owned(),
        password: "$2a$10$averysecrethash".to_owned(),
        first_name: "App".to_owned(),
        last_name: "admin".to_owned(),
        status: Status {
            id: 1,
            status: "Active".to_owned(),
        },
        department: Department {
            id: 1,
            name: "Administration".to_owned(),
            company: Company {
                id: 1,
                name: "Example Company Ltd".to_owned(),
            },
        },
        roles: vec![Role {
            role_id: 3,
            name: "Staff".to_owned(),
        }],
    }
}
