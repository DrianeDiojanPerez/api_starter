use std::sync::Arc;

use async_trait::async_trait;
use uuid::Uuid;

use crate::database::TxManager;
use crate::module::iam::core::domain::{CreateUser, DomainError, User, ACTIVE_USER_STATUS};
use crate::module::iam::core::ports::{UpdateUser, UserRepository, UserService};
use crate::package::errdef::Error;
use crate::package::pagination::{Data, ListRequest};
use crate::package::{utils, validation};

pub struct UserServiceImpl {
    trm: TxManager,
    repository: Arc<dyn UserRepository>,
}

impl UserServiceImpl {
    pub fn new(trm: TxManager, repository: Arc<dyn UserRepository>) -> Self {
        Self { trm, repository }
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Mutex;

    use sqlx::postgres::PgPoolOptions;
    use sqlx::PgConnection;

    use crate::database::Database;
    use crate::module::iam::core::domain::{Company, Department, Role, Status};
    use crate::package::errdef::code;

    /// A pool that never connects. The read paths under test never reach it,
    /// and a test that accidentally opens a transaction fails loudly.
    fn offline_tx_manager() -> TxManager {
        let pool = PgPoolOptions::new()
            .connect_lazy("postgres://offline:offline@127.0.0.1:1/offline")
            .expect("a lazy pool should always build");

        TxManager::new(Arc::new(Database::from_pool(pool)))
    }

    #[derive(Default)]
    struct FakeUserRepository {
        users: Vec<User>,
        total: i64,
        find_error: Option<fn() -> DomainError>,
        seen_requests: Mutex<Vec<ListRequest>>,
    }

    impl FakeUserRepository {
        fn with_users(users: Vec<User>) -> Self {
            Self {
                total: users.len() as i64,
                users,
                ..Self::default()
            }
        }

        fn failing(error: fn() -> DomainError) -> Self {
            Self {
                find_error: Some(error),
                ..Self::default()
            }
        }
    }

    #[async_trait]
    impl UserRepository for FakeUserRepository {
        async fn index(&self, request: &ListRequest) -> Result<(Vec<User>, i64), DomainError> {
            self.seen_requests.lock().unwrap().push(request.clone());
            Ok((self.users.clone(), self.total))
        }

        async fn create(
            &self,
            _tx: &mut PgConnection,
            _user: &CreateUser,
        ) -> Result<Uuid, DomainError> {
            unreachable!("create needs a transaction, covered by the integration tests")
        }

        async fn find_by_id(&self, user_id: Uuid) -> Result<Option<User>, DomainError> {
            if let Some(error) = self.find_error {
                return Err(error());
            }
            Ok(self.users.iter().find(|u| u.id == user_id).cloned())
        }

        async fn find_by_user_name(&self, user_name: &str) -> Result<Option<User>, DomainError> {
            Ok(self
                .users
                .iter()
                .find(|u| u.user_name == user_name)
                .cloned())
        }

        async fn find_by_email(&self, email: &str) -> Result<Option<User>, DomainError> {
            Ok(self.users.iter().find(|u| u.email == email).cloned())
        }

        async fn add_user_roles(
            &self,
            _tx: &mut PgConnection,
            _user_id: Uuid,
            _roles: &[String],
        ) -> Result<(), DomainError> {
            unreachable!("role changes need a transaction")
        }

        async fn remove_user_roles(
            &self,
            _tx: &mut PgConnection,
            _user_id: Uuid,
            _roles: &[String],
        ) -> Result<(), DomainError> {
            unreachable!("role changes need a transaction")
        }

        async fn partial_update(
            &self,
            _tx: &mut PgConnection,
            _user_id: Uuid,
            _fields: &UpdateUser,
        ) -> Result<(), DomainError> {
            unreachable!("updates need a transaction")
        }

        async fn delete(&self, _tx: &mut PgConnection, _user_id: Uuid) -> Result<(), DomainError> {
            unreachable!("deletes need a transaction")
        }
    }

    fn a_user() -> User {
        User {
            id: Uuid::new_v4(),
            user_name: "admin".to_owned(),
            avatar_id: String::new(),
            email: "admin@example.com".to_owned(),
            password: "hashed".to_owned(),
            first_name: "App".to_owned(),
            last_name: "Admin".to_owned(),
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
                role_id: 1,
                name: "Admin".to_owned(),
            }],
        }
    }

    fn service(repository: Arc<FakeUserRepository>) -> UserServiceImpl {
        UserServiceImpl::new(offline_tx_manager(), repository)
    }

    fn code_of(err: &Error) -> i32 {
        match err {
            Error::App(err) => err.code,
            Error::Validation(_) => code::VALIDATION_FAILED,
        }
    }

    #[tokio::test]
    async fn index_wraps_the_rows_in_pagination_metadata() {
        let repository = Arc::new(FakeUserRepository {
            users: vec![a_user()],
            total: 25,
            ..FakeUserRepository::default()
        });
        let service = service(repository.clone());

        let request = ListRequest {
            page: 2,
            per_page: 10,
            ..ListRequest::default()
        };

        let page = service.index(request).await.expect("index should succeed");

        assert_eq!(page.meta.total_count, 25);
        assert_eq!(page.meta.total_pages, 3);
        assert_eq!(page.meta.current_page, 2);
        assert_eq!(page.meta.next_page, Some(3));
        assert_eq!(page.meta.previous_page, Some(1));
    }

    #[tokio::test]
    async fn index_passes_the_filters_through_to_the_repository() {
        let repository = Arc::new(FakeUserRepository::with_users(vec![a_user()]));
        let service = service(repository.clone());

        let request = ListRequest::from_query("page=1&status=Active&role=Admin&sort_by=email");
        service.index(request).await.expect("index should succeed");

        let seen = repository.seen_requests.lock().unwrap();
        let seen = seen.first().expect("the repository should be called");

        assert_eq!(seen.filter("status"), Some("Active"));
        assert_eq!(seen.filter("role"), Some("Admin"));
        assert_eq!(seen.sort_by, "email");
    }

    #[tokio::test]
    async fn find_by_id_returns_the_user() {
        let user = a_user();
        let service = service(Arc::new(FakeUserRepository::with_users(vec![user.clone()])));

        let found = service
            .find_by_id(user.id)
            .await
            .expect("the user should be found");

        assert_eq!(found.id, user.id);
        assert_eq!(found.roles.len(), 1);
    }

    #[tokio::test]
    async fn find_by_id_reports_a_missing_user_as_not_found() {
        let service = service(Arc::new(FakeUserRepository::default()));

        let err = service
            .find_by_id(Uuid::new_v4())
            .await
            .expect_err("the lookup should fail");

        assert_eq!(code_of(&err), code::NOT_FOUND);
        assert!(err.to_string().contains("user does not exists"));
    }

    #[tokio::test]
    async fn a_duplicate_constraint_becomes_an_unprocessable_error() {
        let service = service(Arc::new(FakeUserRepository::failing(|| {
            DomainError::UserDuplicateConstraint
        })));

        let err = service
            .find_by_id(Uuid::new_v4())
            .await
            .expect_err("the lookup should fail");

        assert_eq!(code_of(&err), code::UNPROCESSABLE);
    }

    #[tokio::test]
    async fn an_invalid_status_becomes_a_field_violation() {
        let service = service(Arc::new(FakeUserRepository::failing(|| {
            DomainError::StatusNotFound
        })));

        let err = service
            .find_by_id(Uuid::new_v4())
            .await
            .expect_err("the lookup should fail");

        match err {
            Error::Validation(err) => {
                assert!(err.field_violations.contains_key("status"));
            }
            other => panic!("expected a validation error, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn an_unknown_department_becomes_a_field_violation() {
        let service = service(Arc::new(FakeUserRepository::failing(|| {
            DomainError::DepartmentNotFoundConstraint
        })));

        let err = service
            .find_by_id(Uuid::new_v4())
            .await
            .expect_err("the lookup should fail");

        match err {
            Error::Validation(err) => {
                assert!(err.field_violations.contains_key("department_id"));
            }
            other => panic!("expected a validation error, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn a_weak_password_is_rejected_before_any_transaction_opens() {
        let user = a_user();
        let service = service(Arc::new(FakeUserRepository::with_users(vec![user.clone()])));

        let err = service
            .partial_update(
                user.id,
                UpdateUser {
                    password: Some("weak".to_owned()),
                    ..UpdateUser::default()
                },
            )
            .await
            .expect_err("the update should fail");

        match err {
            Error::Validation(err) => {
                assert!(err.field_violations.contains_key("password"));
            }
            other => panic!("expected a validation error, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn updating_a_missing_user_never_reaches_the_repository() {
        let service = service(Arc::new(FakeUserRepository::default()));

        let err = service
            .partial_update(Uuid::new_v4(), UpdateUser::default())
            .await
            .expect_err("the update should fail");

        assert_eq!(code_of(&err), code::NOT_FOUND);
    }

    #[tokio::test]
    async fn deleting_a_missing_user_never_reaches_the_repository() {
        let service = service(Arc::new(FakeUserRepository::default()));

        let err = service
            .delete(Uuid::new_v4())
            .await
            .expect_err("the delete should fail");

        assert_eq!(code_of(&err), code::NOT_FOUND);
    }
}
