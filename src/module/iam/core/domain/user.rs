use uuid::Uuid;

use super::{Department, Role};

pub const ACTIVE_USER_STATUS: i32 = 1;
pub const DELETED_USER_STATUS: i32 = 2;

#[derive(Debug, Clone)]
pub struct Status {
    pub id: i32,
    pub status: String,
}

/// Status names accepted on filters and partial updates.
pub struct UserStatus;

impl UserStatus {
    pub fn id_of(status: &str) -> Option<i32> {
        match status {
            "Active" => Some(ACTIVE_USER_STATUS),
            "Deleted" => Some(DELETED_USER_STATUS),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct User {
    pub id: Uuid,
    pub user_name: String,
    pub avatar_id: String,
    pub email: String,
    pub password: String,
    pub first_name: String,
    pub last_name: String,
    pub status: Status,
    pub department: Department,
    pub roles: Vec<Role>,
}

#[derive(Debug, Clone)]
pub struct CreateUser {
    pub user_name: String,
    pub avatar_id: String,
    pub email: String,
    pub password: String,
    pub first_name: String,
    pub last_name: String,
    pub status: i32,
    pub department_id: i32,
    pub roles: Vec<String>,
}
