use std::sync::Arc;

use async_trait::async_trait;
use uuid::Uuid;

use crate::database::TxManager;
use crate::module::iam::core::domain::{CreateUser, DomainError, User, ACTIVE_USER_STATUS};
use crate::module::iam::core::ports::{UpdateUser, UserRepository, UserService};
use crate::shared::errdef::Error;
use crate::shared::pagination::{Data, ListRequest};
use crate::shared::{utils, validation};

pub struct UserServiceImpl {
    trm: TxManager,
    repository: Arc<dyn UserRepository>,
}

impl UserServiceImpl {
    pub fn new(trm: TxManager, repository: Arc<dyn UserRepository>) -> Self {
        Self { trm, repository }
    }

    /// Shared translation of repository failures into API errors.
    fn map_domain_error(err: DomainError) -> Error {
        match err {
            DomainError::UserNotFound => Error::not_found("user does not exists"),
            DomainError::UserDuplicateConstraint => {
                Error::unprocessable("username or email is already taken")
            }
            DomainError::StatusNotFound => Error::validation("invalid payload fields")
                .add_violation("status", "field must be of valid type"),
            DomainError::DepartmentNotFoundConstraint => {
                Error::validation("invalid payload fields")
                    .add_violation("department_id", "invalid department id provided")
            }
            DomainError::Database(err) => Error::unknown(err),
        }
    }

    async fn require_user(&self, user_id: Uuid) -> Result<User, Error> {
        self.repository
            .find_by_id(user_id)
            .await
            .map_err(Self::map_domain_error)?
            .ok_or_else(|| Error::not_found("user does not exists"))
    }
}

#[async_trait]
impl UserService for UserServiceImpl {
    async fn index(&self, request: ListRequest) -> Result<Data<User>, Error> {
        tracing::debug!(?request, "Fetching User List");

        let (data, total_records) = self
            .repository
            .index(&request)
            .await
            .map_err(Self::map_domain_error)?;

        Ok(Data::new(
            data,
            total_records,
            request.page,
            request.per_page,
        ))
    }

    async fn create(&self, new_user: CreateUser) -> Result<Uuid, Error> {
        let interim_user = CreateUser {
            password: utils::hash_password(&new_user.password)?,
            status: ACTIVE_USER_STATUS,
            ..new_user
        };

        tracing::debug!(user_name = %interim_user.user_name, "Creating a New User");

        let mut tx = self.trm.begin().await.map_err(Error::unknown)?;

        let user_id = self
            .repository
            .create(&mut tx, &interim_user)
            .await
            .map_err(Self::map_domain_error)?;

        tx.commit().await.map_err(Error::unknown)?;

        Ok(user_id)
    }

    async fn find_by_id(&self, user_id: Uuid) -> Result<User, Error> {
        self.require_user(user_id).await
    }

    async fn partial_update(&self, user_id: Uuid, fields: UpdateUser) -> Result<(), Error> {
        self.require_user(user_id).await?;

        let mut fields = fields;

        if let Some(password) = &fields.password {
            validation::password_checker(password).map_err(|message| {
                Error::validation("invalid payload fields").add_violation("password", message)
            })?;

            fields.password = Some(utils::hash_password(password)?);
        }

        let mut tx = self.trm.begin().await.map_err(Error::unknown)?;

        self.repository
            .partial_update(&mut tx, user_id, &fields)
            .await
            .map_err(Self::map_domain_error)?;

        if let Some(roles) = fields.add_roles.as_deref() {
            self.repository
                .add_user_roles(&mut tx, user_id, roles)
                .await
                .map_err(Self::map_domain_error)?;
        }

        if let Some(roles) = fields.remove_roles.as_deref() {
            self.repository
                .remove_user_roles(&mut tx, user_id, roles)
                .await
                .map_err(Self::map_domain_error)?;
        }

        tx.commit().await.map_err(Error::unknown)?;

        Ok(())
    }

    async fn delete(&self, user_id: Uuid) -> Result<(), Error> {
        self.require_user(user_id).await?;

        let mut tx = self.trm.begin().await.map_err(Error::unknown)?;

        self.repository
            .delete(&mut tx, user_id)
            .await
            .map_err(Self::map_domain_error)?;

        tx.commit().await.map_err(Error::unknown)?;

        Ok(())
    }
}
