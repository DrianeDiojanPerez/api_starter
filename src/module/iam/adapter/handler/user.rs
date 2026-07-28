use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json as AxumJson;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::module::iam::core::domain::{self as domain};
use crate::module::iam::core::ports::{UpdateUser, UserService};
use crate::shared::auth::AuthUser;
use crate::shared::errdef::Error;
use crate::shared::extract::{Json, ValidatedJson};
use crate::shared::pagination::ListRequest;
use crate::shared::response::{self, Response};
use crate::shared::validation::strong_password;

#[derive(Debug, Clone, Serialize)]
pub struct Company {
    pub company_id: i32,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Department {
    pub department_id: i32,
    pub department_name: String,
    pub company: Company,
}

#[derive(Debug, Clone, Serialize)]
pub struct User {
    pub user_id: Uuid,
    pub avatar_id: String,
    pub user_name: String,
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub status: String,
    pub department: Department,
    pub roles: Vec<String>,
}

impl From<domain::User> for User {
    fn from(user: domain::User) -> Self {
        Self {
            user_id: user.id,
            avatar_id: user.avatar_id,
            user_name: user.user_name,
            email: user.email,
            first_name: user.first_name,
            last_name: user.last_name,
            status: user.status.status,
            department: Department {
                department_id: user.department.id,
                department_name: user.department.name,
                company: Company {
                    company_id: user.department.company.id,
                    name: user.department.company.name,
                },
            },
            roles: user.roles.into_iter().map(|role| role.name).collect(),
        }
    }
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateUserRequest {
    #[validate(length(min = 1, message = "field is required and cannot be empty"))]
    pub user_name: String,
    #[serde(default)]
    pub avatar_id: String,
    #[validate(email)]
    pub email: String,
    #[validate(custom(function = "strong_password"))]
    pub password: String,
    #[validate(length(min = 1, message = "field is required and cannot be empty"))]
    pub first_name: String,
    #[validate(length(min = 1, message = "field is required and cannot be empty"))]
    pub last_name: String,
    #[validate(range(min = 1))]
    pub department: i32,
    #[serde(default)]
    pub roles: Vec<String>,
}

pub type UserState = Arc<dyn UserService>;

#[tracing::instrument(name = "UserService.Index", skip_all)]
pub async fn index(
    State(service): State<UserState>,
    request: ListRequest,
) -> Result<AxumJson<Response<Vec<User>>>, Error> {
    let page = service.index(request).await?.map(User::from);

    Ok(response::ok_paginated(page.data, page.meta))
}

#[tracing::instrument(name = "UserService.Create", skip_all)]
pub async fn create(
    State(service): State<UserState>,
    ValidatedJson(payload): ValidatedJson<CreateUserRequest>,
) -> Result<AxumJson<Response<Uuid>>, Error> {
    let user_id = service
        .create(domain::CreateUser {
            user_name: payload.user_name,
            avatar_id: payload.avatar_id,
            email: payload.email,
            password: payload.password,
            first_name: payload.first_name,
            last_name: payload.last_name,
            status: domain::ACTIVE_USER_STATUS,
            department_id: payload.department,
            roles: payload.roles,
        })
        .await?;

    Ok(response::ok(user_id))
}

#[tracing::instrument(name = "UserService.Get", skip_all)]
pub async fn get(
    State(service): State<UserState>,
    Path(user_id): Path<Uuid>,
) -> Result<AxumJson<Response<User>>, Error> {
    let user = service.find_by_id(user_id).await?;

    Ok(response::ok(User::from(user)))
}

#[tracing::instrument(name = "UserService.GetMyUser", skip_all)]
pub async fn get_my_user(
    State(service): State<UserState>,
    auth_user: AuthUser,
) -> Result<AxumJson<Response<User>>, Error> {
    let user = service.find_by_id(auth_user.id()).await?;

    Ok(response::ok(User::from(user)))
}

#[tracing::instrument(name = "UserService.PartialUpdate", skip_all)]
pub async fn patch(
    State(service): State<UserState>,
    Path(user_id): Path<Uuid>,
    Json(payload): Json<UpdateUser>,
) -> Result<AxumJson<Response<&'static str>>, Error> {
    service.partial_update(user_id, payload).await?;

    Ok(response::ok("successfully update user"))
}

#[tracing::instrument(name = "UserService.PartialUpdateMyUser", skip_all)]
pub async fn patch_my_user(
    State(service): State<UserState>,
    auth_user: AuthUser,
    Json(payload): Json<UpdateUser>,
) -> Result<AxumJson<Response<&'static str>>, Error> {
    service
        .partial_update(auth_user.id(), payload.restricted_to_self())
        .await?;

    Ok(response::ok("successfully update user"))
}

#[derive(Debug, Serialize)]
pub struct DeletedUser {
    pub message: &'static str,
    pub user_id: Uuid,
}

#[tracing::instrument(name = "UserService.Delete", skip_all)]
pub async fn delete(
    State(service): State<UserState>,
    Path(user_id): Path<Uuid>,
) -> Result<AxumJson<Response<DeletedUser>>, Error> {
    service.delete(user_id).await?;

    Ok(response::ok(DeletedUser {
        message: "User deleted successfully",
        user_id,
    }))
}
