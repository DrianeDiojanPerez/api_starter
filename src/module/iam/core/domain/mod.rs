mod company;
mod department;
mod permission;
mod role;
mod user;

pub use company::Company;
pub use department::Department;
pub use permission::{AddPermission, Permission, RemovePermission};
pub use role::Role;
pub use user::{CreateUser, Status, User, UserStatus, ACTIVE_USER_STATUS, DELETED_USER_STATUS};

/// Failures the repositories raise and the services translate into API errors.
#[derive(Debug, thiserror::Error)]
pub enum DomainError {
    #[error("user does not exists")]
    UserNotFound,
    #[error("column is taken")]
    UserDuplicateConstraint,
    #[error("invalid user status")]
    StatusNotFound,
    #[error("invalid department id")]
    DepartmentNotFoundConstraint,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}
