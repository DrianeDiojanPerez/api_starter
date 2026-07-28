//! Repository and transaction tests against a real PostgreSQL instance.
//!
//! These are skipped unless `TEST_DATABASE_URL` is set, so `cargo test` stays
//! runnable without any infrastructure. Run them with:
//!
//! ```text
//! just test-all
//! ```
//!
//! The migrations are embedded in the crate, so an empty database is enough:
//! the first test to run applies them.
//!
//! Every test namespaces the rows it creates, so they are safe to run in
//! parallel and against a database that already holds the seed data.

use std::sync::Arc;

use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use api_starter::database::{Database, TxManager};
use api_starter::module::iam::adapter::repository::{PgPermissionRepository, PgUserRepository};
use api_starter::module::iam::core::domain::{
    CreateUser, DomainError, ACTIVE_USER_STATUS, DELETED_USER_STATUS,
};
use api_starter::module::iam::core::ports::{
    PermissionRepository, UpdateUser, UserRepository, UserService,
};
use api_starter::module::iam::core::service::UserServiceImpl;
use api_starter::shared::auth::{PostgresAuthStore, Store as AuthStore};
use api_starter::shared::pagination::ListRequest;
use api_starter::shared::rbac::{Engine, PostgresRbacStore, RbacEngine, Store as RbacStore};
use api_starter::shared::utils;

/// Returns a migrated pool, or `None` when the suite should be skipped.
///
/// Each test gets its own pool: `#[tokio::test]` builds a runtime per test,
/// and a sqlx pool cannot outlive the runtime that created it. Migrating is
/// idempotent and sqlx takes an advisory lock first, so running it every time
/// is safe and costs one round trip once the schema is up to date.
async fn pool() -> Option<PgPool> {
    let url = std::env::var("TEST_DATABASE_URL").ok()?;

    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&url)
        .await
        .expect("TEST_DATABASE_URL is set but the database is unreachable");

    api_starter::database::MIGRATOR
        .run(&pool)
        .await
        .expect("the embedded migrations should apply");

    Some(pool)
}

/// Skips the test body when there is no database to talk to.
macro_rules! database {
    () => {
        match pool().await {
            Some(pool) => Fixture::new(pool),
            None => {
                eprintln!("skipping: TEST_DATABASE_URL is not set");
                return;
            }
        }
    };
}

struct Fixture {
    db: Arc<Database>,
    users: PgUserRepository,
    permissions: PgPermissionRepository,
    /// Suffix that keeps this test's rows apart from every other test's.
    tag: String,
}

impl Fixture {
    fn new(pool: PgPool) -> Self {
        let db = Arc::new(Database::from_pool(pool));

        Self {
            users: PgUserRepository::new(db.clone()),
            permissions: PgPermissionRepository::new(db.clone()),
            tag: Uuid::new_v4().simple().to_string()[..12].to_owned(),
            db,
        }
    }

    fn tx_manager(&self) -> TxManager {
        TxManager::new(self.db.clone())
    }

    fn user_service(&self) -> UserServiceImpl {
        UserServiceImpl::new(
            self.tx_manager(),
            Arc::new(PgUserRepository::new(self.db.clone())),
        )
    }

    fn a_new_user(&self, name: &str) -> CreateUser {
        CreateUser {
            user_name: format!("{name}_{}", self.tag),
            avatar_id: String::new(),
            email: format!("{name}_{}@test.local", self.tag),
            password: "hashed-by-the-service".to_owned(),
            first_name: "Test".to_owned(),
            last_name: "User".to_owned(),
            status: ACTIVE_USER_STATUS,
            department_id: 1,
            roles: Vec::new(),
        }
    }

    /// Inserts a user through the repository, committing the transaction.
    async fn insert(&self, user: &CreateUser) -> Uuid {
        let mut tx = self.tx_manager().begin().await.expect("begin should work");
        let id = self
            .users
            .create(&mut tx, user)
            .await
            .expect("the insert should succeed");
        tx.commit().await.expect("commit should work");
        id
    }
}

// ──── User repository ──────────────────────────────────

#[tokio::test]
async fn creates_a_user_with_its_roles_and_reads_it_back() {
    let fx = database!();

    let mut new_user = fx.a_new_user("creates");
    new_user.roles = vec!["Staff".to_owned(), "Developer".to_owned()];

    let user_id = fx.insert(&new_user).await;

    let found = fx
        .users
        .find_by_id(user_id)
        .await
        .expect("the lookup should succeed")
        .expect("the user should exist");

    assert_eq!(found.user_name, new_user.user_name);
    assert_eq!(found.email, new_user.email);
    assert_eq!(found.status.status, "Active");
    assert_eq!(found.department.name, "Administration");
    assert_eq!(found.department.company.name, "Example Company Ltd");

    let mut roles: Vec<_> = found.roles.iter().map(|r| r.name.clone()).collect();
    roles.sort();
    assert_eq!(roles, vec!["Developer".to_owned(), "Staff".to_owned()]);
}

#[tokio::test]
async fn finds_a_user_by_user_name_and_by_email() {
    let fx = database!();

    let new_user = fx.a_new_user("lookup");
    let user_id = fx.insert(&new_user).await;

    let by_name = fx
        .users
        .find_by_user_name(&new_user.user_name)
        .await
        .expect("the lookup should succeed")
        .expect("the user should exist");
    let by_email = fx
        .users
        .find_by_email(&new_user.email)
        .await
        .expect("the lookup should succeed")
        .expect("the user should exist");

    assert_eq!(by_name.id, user_id);
    assert_eq!(by_email.id, user_id);
}

#[tokio::test]
async fn a_missing_user_is_none_rather_than_an_error() {
    let fx = database!();

    let found = fx
        .users
        .find_by_id(Uuid::new_v4())
        .await
        .expect("the lookup should succeed");

    assert!(found.is_none());
}

#[tokio::test]
async fn a_duplicate_user_name_is_reported_as_a_domain_error() {
    let fx = database!();

    let new_user = fx.a_new_user("duplicate");
    fx.insert(&new_user).await;

    let mut tx = fx.tx_manager().begin().await.expect("begin should work");
    let err = fx
        .users
        .create(&mut tx, &new_user)
        .await
        .expect_err("the second insert should fail");

    assert!(matches!(err, DomainError::UserDuplicateConstraint));
}

#[tokio::test]
async fn an_unknown_department_is_reported_as_a_domain_error() {
    let fx = database!();

    let mut new_user = fx.a_new_user("no_department");
    new_user.department_id = 987_654;

    let mut tx = fx.tx_manager().begin().await.expect("begin should work");
    let err = fx
        .users
        .create(&mut tx, &new_user)
        .await
        .expect_err("the insert should fail");

    assert!(matches!(err, DomainError::DepartmentNotFoundConstraint));
}

#[tokio::test]
async fn adding_the_same_role_twice_is_idempotent() {
    let fx = database!();

    let new_user = fx.a_new_user("roles_idempotent");
    let user_id = fx.insert(&new_user).await;

    for _ in 0..2 {
        let mut tx = fx.tx_manager().begin().await.expect("begin should work");
        fx.users
            .add_user_roles(&mut tx, user_id, &["Staff".to_owned()])
            .await
            .expect("adding a role should succeed");
        tx.commit().await.expect("commit should work");
    }

    let found = fx
        .users
        .find_by_id(user_id)
        .await
        .expect("the lookup should succeed")
        .expect("the user should exist");

    assert_eq!(found.roles.len(), 1);
}

#[tokio::test]
async fn removes_only_the_named_roles() {
    let fx = database!();

    let mut new_user = fx.a_new_user("roles_remove");
    new_user.roles = vec!["Staff".to_owned(), "Developer".to_owned()];
    let user_id = fx.insert(&new_user).await;

    let mut tx = fx.tx_manager().begin().await.expect("begin should work");
    fx.users
        .remove_user_roles(&mut tx, user_id, &["Staff".to_owned()])
        .await
        .expect("removing a role should succeed");
    tx.commit().await.expect("commit should work");

    let found = fx
        .users
        .find_by_id(user_id)
        .await
        .expect("the lookup should succeed")
        .expect("the user should exist");

    assert_eq!(found.roles.len(), 1);
    assert_eq!(found.roles[0].name, "Developer");
}

#[tokio::test]
async fn an_unknown_role_name_is_silently_ignored() {
    let fx = database!();

    let new_user = fx.a_new_user("roles_unknown");
    let user_id = fx.insert(&new_user).await;

    let mut tx = fx.tx_manager().begin().await.expect("begin should work");
    fx.users
        .add_user_roles(&mut tx, user_id, &["NotARole".to_owned()])
        .await
        .expect("the insert should not fail");
    tx.commit().await.expect("commit should work");

    let found = fx
        .users
        .find_by_id(user_id)
        .await
        .expect("the lookup should succeed")
        .expect("the user should exist");

    assert!(found.roles.is_empty());
}

#[tokio::test]
async fn updates_only_the_columns_present_in_the_payload() {
    let fx = database!();

    let new_user = fx.a_new_user("partial_update");
    let user_id = fx.insert(&new_user).await;

    let mut tx = fx.tx_manager().begin().await.expect("begin should work");
    fx.users
        .partial_update(
            &mut tx,
            user_id,
            &UpdateUser {
                first_name: Some("Changed".to_owned()),
                avatar_id: Some("avatar.gif".to_owned()),
                ..UpdateUser::default()
            },
        )
        .await
        .expect("the update should succeed");
    tx.commit().await.expect("commit should work");

    let found = fx
        .users
        .find_by_id(user_id)
        .await
        .expect("the lookup should succeed")
        .expect("the user should exist");

    assert_eq!(found.first_name, "Changed");
    assert_eq!(found.avatar_id, "avatar.gif");
    assert_eq!(
        found.last_name, "User",
        "untouched columns keep their value"
    );
    assert_eq!(found.email, new_user.email);
}

#[tokio::test]
async fn an_invalid_status_name_never_reaches_the_database() {
    let fx = database!();

    let new_user = fx.a_new_user("bad_status");
    let user_id = fx.insert(&new_user).await;

    let mut tx = fx.tx_manager().begin().await.expect("begin should work");
    let err = fx
        .users
        .partial_update(
            &mut tx,
            user_id,
            &UpdateUser {
                status: Some("Imaginary".to_owned()),
                ..UpdateUser::default()
            },
        )
        .await
        .expect_err("the update should fail");

    assert!(matches!(err, DomainError::StatusNotFound));
}

#[tokio::test]
async fn a_payload_with_no_column_changes_runs_no_statement() {
    let fx = database!();

    let new_user = fx.a_new_user("noop_update");
    let user_id = fx.insert(&new_user).await;

    let mut tx = fx.tx_manager().begin().await.expect("begin should work");
    fx.users
        .partial_update(
            &mut tx,
            user_id,
            &UpdateUser {
                add_roles: Some(vec!["Staff".to_owned()]),
                ..UpdateUser::default()
            },
        )
        .await
        .expect("an empty update should be a no op, not a broken statement");
    tx.commit().await.expect("commit should work");
}

#[tokio::test]
async fn deleting_a_user_only_flips_its_status() {
    let fx = database!();

    let new_user = fx.a_new_user("soft_delete");
    let user_id = fx.insert(&new_user).await;

    let mut tx = fx.tx_manager().begin().await.expect("begin should work");
    fx.users
        .delete(&mut tx, user_id)
        .await
        .expect("the delete should succeed");
    tx.commit().await.expect("commit should work");

    let found = fx
        .users
        .find_by_id(user_id)
        .await
        .expect("the lookup should succeed")
        .expect("the row is kept, only its status changes");

    assert_eq!(found.status.id, DELETED_USER_STATUS);
    assert_eq!(found.status.status, "Deleted");
}

// ──── Index, filters and pagination ──────────────────────────────────

#[tokio::test]
async fn the_index_filters_by_role() {
    let fx = database!();

    let mut with_role = fx.a_new_user("filter_role_yes");
    with_role.roles = vec!["Developer".to_owned()];
    fx.insert(&with_role).await;

    let without_role = fx.a_new_user("filter_role_no");
    fx.insert(&without_role).await;

    let request = ListRequest::from_query("per_page=100&role=Developer");
    let (users, _) = fx
        .users
        .index(&request)
        .await
        .expect("the index should succeed");

    let names: Vec<_> = users.iter().map(|u| u.user_name.clone()).collect();

    assert!(names.contains(&with_role.user_name));
    assert!(!names.contains(&without_role.user_name));
}

#[tokio::test]
async fn the_index_filters_by_status() {
    let fx = database!();

    let new_user = fx.a_new_user("filter_status");
    let user_id = fx.insert(&new_user).await;

    let mut tx = fx.tx_manager().begin().await.expect("begin should work");
    fx.users
        .delete(&mut tx, user_id)
        .await
        .expect("the delete should succeed");
    tx.commit().await.expect("commit should work");

    let (active, _) = fx
        .users
        .index(&ListRequest::from_query("per_page=100&status=Active"))
        .await
        .expect("the index should succeed");
    let (deleted, _) = fx
        .users
        .index(&ListRequest::from_query("per_page=100&status=Deleted"))
        .await
        .expect("the index should succeed");

    assert!(!active.iter().any(|u| u.id == user_id));
    assert!(deleted.iter().any(|u| u.id == user_id));
}

#[tokio::test]
async fn the_index_filters_by_a_partial_first_name() {
    let fx = database!();

    let mut new_user = fx.a_new_user("filter_first_name");
    new_user.first_name = format!("Zebediah{}", fx.tag);
    fx.insert(&new_user).await;

    let query = format!("per_page=100&first_name=ebediah{}", fx.tag);
    let (users, total) = fx
        .users
        .index(&ListRequest::from_query(&query))
        .await
        .expect("the index should succeed");

    assert_eq!(total, 1);
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].user_name, new_user.user_name);
}

#[tokio::test]
async fn an_unknown_filter_is_ignored_rather_than_applied() {
    let fx = database!();

    let mut new_user = fx.a_new_user("unknown_filter");
    new_user.first_name = format!("Unfiltered{}", fx.tag);
    fx.insert(&new_user).await;

    // Scoped to this test's own rows, so a parallel test inserting users
    // cannot move the count underneath it.
    let base = format!("per_page=100&first_name=Unfiltered{}", fx.tag);

    let (_, with_nonsense) = fx
        .users
        .index(&ListRequest::from_query(&format!("{base}&nonsense=1")))
        .await
        .expect("the index should succeed");
    let (_, without) = fx
        .users
        .index(&ListRequest::from_query(&base))
        .await
        .expect("the index should succeed");

    assert_eq!(without, 1);
    assert_eq!(with_nonsense, without);
}

#[tokio::test]
async fn the_index_counts_every_match_but_only_returns_one_page() {
    let fx = database!();

    for index in 0..3 {
        let mut user = fx.a_new_user(&format!("page_{index}"));
        user.first_name = format!("Paged{}", fx.tag);
        fx.insert(&user).await;
    }

    let query = format!("per_page=2&first_name=Paged{}", fx.tag);
    let (users, total) = fx
        .users
        .index(&ListRequest::from_query(&query))
        .await
        .expect("the index should succeed");

    assert_eq!(total, 3, "the count ignores the limit");
    assert_eq!(users.len(), 2, "the page respects it");
}

#[tokio::test]
async fn the_index_sorts_by_the_requested_column() {
    let fx = database!();

    for name in ["c", "a", "b"] {
        let mut user = fx.a_new_user(&format!("sort_{name}"));
        user.first_name = format!("Sorted{}", fx.tag);
        user.email = format!("{name}_{}@sorted.local", fx.tag);
        fx.insert(&user).await;
    }

    let query = format!(
        "per_page=100&first_name=Sorted{}&sort_by=email&order=asc",
        fx.tag
    );
    let (users, _) = fx
        .users
        .index(&ListRequest::from_query(&query))
        .await
        .expect("the index should succeed");

    let emails: Vec<_> = users.iter().map(|u| u.email.clone()).collect();
    let mut sorted = emails.clone();
    sorted.sort();

    assert_eq!(emails, sorted);
}

// ──── Transactions ──────────────────────────────────

#[tokio::test]
async fn a_rolled_back_transaction_leaves_nothing_behind() {
    let fx = database!();

    let new_user = fx.a_new_user("rollback");

    let mut tx = fx.tx_manager().begin().await.expect("begin should work");
    let user_id = fx
        .users
        .create(&mut tx, &new_user)
        .await
        .expect("the insert should succeed inside the transaction");
    tx.rollback().await.expect("rollback should work");

    assert!(fx
        .users
        .find_by_id(user_id)
        .await
        .expect("the lookup should succeed")
        .is_none());
}

#[tokio::test]
async fn a_failed_role_assignment_rolls_the_whole_user_back() {
    let fx = database!();

    let new_user = fx.a_new_user("atomic");

    let mut tx = fx.tx_manager().begin().await.expect("begin should work");
    let user_id = fx
        .users
        .create(&mut tx, &new_user)
        .await
        .expect("the insert should succeed");

    // A statement that is guaranteed to fail, poisoning the transaction.
    let failed = sqlx::query("INSERT INTO iam.user_has_roles (user_id, role_id) VALUES ($1, $2)")
        .bind(user_id)
        .bind(987_654)
        .execute(&mut *tx)
        .await;

    assert!(failed.is_err(), "the foreign key should reject this");
    tx.rollback().await.expect("rollback should work");

    assert!(fx
        .users
        .find_by_id(user_id)
        .await
        .expect("the lookup should succeed")
        .is_none());
}

// ──── User service over a real database ──────────────────────────────────

#[tokio::test]
async fn the_service_hashes_the_password_before_storing_it() {
    let fx = database!();

    let mut new_user = fx.a_new_user("service_create");
    new_user.password = "Sup3r$ecret".to_owned();

    let user_id = fx
        .user_service()
        .create(new_user)
        .await
        .expect("the create should succeed");

    let stored = fx
        .users
        .find_by_id(user_id)
        .await
        .expect("the lookup should succeed")
        .expect("the user should exist");

    assert_ne!(stored.password, "Sup3r$ecret");
    assert!(utils::compare_hash_and_password(&stored.password, "Sup3r$ecret").is_ok());
}

#[tokio::test]
async fn the_service_applies_the_column_and_role_changes_together() {
    let fx = database!();

    let mut new_user = fx.a_new_user("service_patch");
    new_user.roles = vec!["Staff".to_owned()];
    let user_id = fx.insert(&new_user).await;

    fx.user_service()
        .partial_update(
            user_id,
            UpdateUser {
                first_name: Some("Patched".to_owned()),
                add_roles: Some(vec!["Developer".to_owned()]),
                remove_roles: Some(vec!["Staff".to_owned()]),
                ..UpdateUser::default()
            },
        )
        .await
        .expect("the update should succeed");

    let found = fx
        .users
        .find_by_id(user_id)
        .await
        .expect("the lookup should succeed")
        .expect("the user should exist");

    assert_eq!(found.first_name, "Patched");
    assert_eq!(found.roles.len(), 1);
    assert_eq!(found.roles[0].name, "Developer");
}

#[tokio::test]
async fn the_service_hashes_a_password_supplied_through_a_patch() {
    let fx = database!();

    let new_user = fx.a_new_user("service_patch_password");
    let user_id = fx.insert(&new_user).await;

    fx.user_service()
        .partial_update(
            user_id,
            UpdateUser {
                password: Some("An0ther@Pass".to_owned()),
                ..UpdateUser::default()
            },
        )
        .await
        .expect("the update should succeed");

    let found = fx
        .users
        .find_by_id(user_id)
        .await
        .expect("the lookup should succeed")
        .expect("the user should exist");

    assert!(utils::compare_hash_and_password(&found.password, "An0ther@Pass").is_ok());
}

// ──── Auth store ──────────────────────────────────

#[tokio::test]
async fn the_auth_store_reads_a_user_with_its_roles() {
    let fx = database!();

    let mut new_user = fx.a_new_user("auth_store");
    new_user.roles = vec!["Staff".to_owned()];
    let user_id = fx.insert(&new_user).await;

    let store = PostgresAuthStore::new(fx.db.clone());

    let by_id = store
        .find_user_by_id(user_id)
        .await
        .expect("the lookup should succeed")
        .expect("the user should exist");
    let by_email = store
        .find_user_by_email(&new_user.email)
        .await
        .expect("the lookup should succeed")
        .expect("the user should exist");

    assert_eq!(by_id.id, user_id);
    assert_eq!(by_id.roles, vec!["Staff".to_owned()]);
    assert_eq!(by_email.id, user_id);
    assert!(!by_id.password.is_empty(), "the hash is needed to log in");
}

#[tokio::test]
async fn the_auth_store_round_trips_a_password_reset() {
    let fx = database!();

    let new_user = fx.a_new_user("auth_reset");
    fx.insert(&new_user).await;

    let store = PostgresAuthStore::new(fx.db.clone());
    let digest = utils::hash_token(&format!("raw-{}", fx.tag));

    store
        .create_password_reset(&new_user.email, &digest)
        .await
        .expect("the insert should succeed");

    let found = store
        .find_password_by_token(&digest)
        .await
        .expect("the lookup should succeed")
        .expect("the reset should exist");

    assert_eq!(found.email, new_user.email);
    assert_eq!(found.token, digest);

    store
        .reset_password(&new_user.email, "a-new-hash")
        .await
        .expect("the update should succeed");
    store
        .delete_password_reset(&new_user.email)
        .await
        .expect("the delete should succeed");

    assert!(store
        .find_password_by_token(&digest)
        .await
        .expect("the lookup should succeed")
        .is_none());

    let updated = store
        .find_user_by_email(&new_user.email)
        .await
        .expect("the lookup should succeed")
        .expect("the user should exist");
    assert_eq!(updated.password, "a-new-hash");
}

// ──── RBAC over a real database ──────────────────────────────────

#[tokio::test]
async fn the_rbac_store_reads_the_roles_and_permissions_of_a_user() {
    let fx = database!();

    let mut new_user = fx.a_new_user("rbac_store");
    new_user.roles = vec!["Developer".to_owned()];
    let user_id = fx.insert(&new_user).await;

    let store = PostgresRbacStore::new(fx.db.clone());

    let roles = store
        .get_roles(user_id)
        .await
        .expect("the lookup should succeed");
    assert_eq!(roles, vec!["Developer".to_owned()]);

    let permissions = store
        .get_permissions(user_id)
        .await
        .expect("the lookup should succeed");
    assert!(
        permissions
            .iter()
            .any(|p| p.resource == "Users" && p.name == "View All"),
        "the Developer role is seeded with every permission"
    );

    assert!(store
        .has_permission(user_id, "Users", "View All")
        .await
        .expect("the check should succeed"));
    assert!(!store
        .has_permission(user_id, "Users", "Not A Permission")
        .await
        .expect("the check should succeed"));
}

#[tokio::test]
async fn the_engine_grants_a_permission_that_comes_from_a_role() {
    let fx = database!();

    let mut new_user = fx.a_new_user("rbac_engine");
    new_user.roles = vec!["Developer".to_owned()];
    let user_id = fx.insert(&new_user).await;

    let engine = RbacEngine::new("Admin", Arc::new(PostgresRbacStore::new(fx.db.clone())));

    assert!(engine.can(user_id, "Users.View All").await);
    assert!(!engine.can(user_id, "Users.Not A Permission").await);
}

#[tokio::test]
async fn the_engine_denies_a_user_with_no_roles_at_all() {
    let fx = database!();

    let user_id = fx.insert(&fx.a_new_user("rbac_no_roles")).await;

    let engine = RbacEngine::new("Admin", Arc::new(PostgresRbacStore::new(fx.db.clone())));

    assert!(!engine.can(user_id, "Users.View All").await);
}

#[tokio::test]
async fn the_super_role_is_granted_everything_including_unseeded_actions() {
    let fx = database!();

    let mut new_user = fx.a_new_user("rbac_admin");
    new_user.roles = vec!["Admin".to_owned()];
    let user_id = fx.insert(&new_user).await;

    let engine = RbacEngine::new("Admin", Arc::new(PostgresRbacStore::new(fx.db.clone())));

    assert!(engine.can(user_id, "Anything.At All").await);
}

// ──── Permission repository ──────────────────────────────────

#[tokio::test]
async fn lists_the_seeded_permissions_with_their_module() {
    let fx = database!();

    let permissions = fx
        .permissions
        .list_all()
        .await
        .expect("the listing should succeed");

    let view_all = permissions
        .iter()
        .find(|p| p.resource == "Users" && p.name == "View All")
        .expect("the seed data should contain Users.View All");

    assert_eq!(view_all.module, "IAM Module");
}

#[tokio::test]
async fn lists_the_permissions_attached_to_a_role() {
    let fx = database!();

    let admin_role_id: i32 = sqlx::query("SELECT id FROM iam.roles WHERE name = 'Admin'")
        .fetch_one(fx.db.pool())
        .await
        .expect("the Admin role should be seeded")
        .try_get("id")
        .expect("the id should be readable");

    let permissions = fx
        .permissions
        .list_permissions_by_role(admin_role_id)
        .await
        .expect("the listing should succeed");

    assert!(!permissions.is_empty());
    assert!(permissions.iter().any(|p| p.resource == "Users"));
}

#[tokio::test]
async fn a_role_without_permissions_lists_nothing() {
    let fx = database!();

    let permissions = fx
        .permissions
        .list_permissions_by_role(987_654)
        .await
        .expect("the listing should succeed");

    assert!(permissions.is_empty());
}
