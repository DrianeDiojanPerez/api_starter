use std::sync::Arc;

use async_trait::async_trait;
use sqlx::postgres::PgRow;
use sqlx::{PgConnection, QueryBuilder, Row};
use uuid::Uuid;

use crate::database::{pg_error, Database};
use crate::module::iam::core::domain::{
    Company, CreateUser, Department, DomainError, Role, Status, User, UserStatus,
    DELETED_USER_STATUS,
};
use crate::module::iam::core::ports::{UpdateUser, UserRepository};
use crate::shared::pagination::ListRequest;

const SELECT_USER: &str = "SELECT u.id, u.user_name, u.avatar_id, u.email, u.password, \
     u.first_name, u.last_name, us.id AS status_id, us.status, \
     d.id AS department_id, d.name AS department_name, c.id AS company_id, c.name AS company_name \
     FROM iam.users u \
     INNER JOIN iam.user_statuses us ON u.status_id = us.id \
     INNER JOIN iam.departments d ON u.department_id = d.id \
     INNER JOIN iam.companies c ON d.company_id = c.id";

const COUNT_USER: &str = "SELECT COUNT(*) AS total FROM iam.users u \
     INNER JOIN iam.user_statuses us ON u.status_id = us.id \
     INNER JOIN iam.departments d ON u.department_id = d.id \
     INNER JOIN iam.companies c ON d.company_id = c.id";

/// Sortable columns, mapped from the public name to the qualified column so a
/// query parameter can never reach the SQL text unchecked.
fn sort_column(sort_by: &str) -> &'static str {
    match sort_by {
        "email" => "u.email",
        "first_name" => "u.first_name",
        "last_name" => "u.last_name",
        "status" => "us.status",
        "department" => "d.name",
        _ => "u.user_name",
    }
}

fn sort_order(order: &str) -> &'static str {
    if order == "asc" {
        "asc"
    } else {
        "desc"
    }
}

pub struct PgUserRepository {
    db: Arc<Database>,
}

impl PgUserRepository {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// Appends the supported `WHERE` clauses. Unknown filters are ignored,
    /// matching the eligible filter map of the Go repository.
    fn push_filters<'a>(builder: &mut QueryBuilder<'a, sqlx::Postgres>, request: &'a ListRequest) {
        if let Some(status) = request.filter("status") {
            if let Some(status_id) = UserStatus::id_of(status) {
                builder.push(" AND us.id = ").push_bind(status_id);
            }
        }

        if let Some(department_id) = request
            .filter("department_id")
            .and_then(|value| value.parse::<i32>().ok())
        {
            builder
                .push(" AND u.department_id = ")
                .push_bind(department_id);
        }

        if let Some(role) = request.filter("role") {
            builder
                .push(
                    " AND EXISTS (SELECT 1 FROM iam.user_has_roles uhr \
                     INNER JOIN iam.roles r ON uhr.role_id = r.id \
                     WHERE uhr.user_id = u.id AND r.name = ",
                )
                .push_bind(role)
                .push(")");
        }

        if let Some(first_name) = request.filter("first_name") {
            builder
                .push(" AND u.first_name ILIKE ")
                .push_bind(format!("%{first_name}%"));
        }
    }

    fn map_user(row: &PgRow) -> Result<User, sqlx::Error> {
        Ok(User {
            id: row.try_get("id")?,
            user_name: row.try_get("user_name")?,
            avatar_id: row.try_get("avatar_id")?,
            email: row.try_get("email")?,
            password: row.try_get("password")?,
            first_name: row.try_get("first_name")?,
            last_name: row.try_get("last_name")?,
            status: Status {
                id: row.try_get("status_id")?,
                status: row.try_get("status")?,
            },
            department: Department {
                id: row.try_get("department_id")?,
                name: row.try_get("department_name")?,
                company: Company {
                    id: row.try_get("company_id")?,
                    name: row.try_get("company_name")?,
                },
            },
            roles: Vec::new(),
        })
    }

    async fn get_user_roles(&self, user_id: Uuid) -> Result<Vec<Role>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT r.id, r.name FROM iam.roles r \
             INNER JOIN iam.user_has_roles uhr ON r.id = uhr.role_id \
             WHERE uhr.user_id = $1",
        )
        .bind(user_id)
        .fetch_all(self.db.pool())
        .await?;

        rows.iter()
            .map(|row| {
                Ok(Role {
                    role_id: row.try_get("id")?,
                    name: row.try_get("name")?,
                })
            })
            .collect()
    }

    /// Maps a single row and loads its roles.
    async fn hydrate(&self, row: Option<PgRow>) -> Result<Option<User>, DomainError> {
        let Some(row) = row else {
            return Ok(None);
        };

        let mut user = Self::map_user(&row)?;
        user.roles = self.get_user_roles(user.id).await?;

        Ok(Some(user))
    }
}

#[async_trait]
impl UserRepository for PgUserRepository {
    async fn index(&self, request: &ListRequest) -> Result<(Vec<User>, i64), DomainError> {
        let mut count_builder = QueryBuilder::new(COUNT_USER);
        count_builder.push(" WHERE TRUE");
        Self::push_filters(&mut count_builder, request);

        let total_count: i64 = count_builder
            .build()
            .fetch_one(self.db.pool())
            .await?
            .try_get("total")?;

        let mut select_builder = QueryBuilder::new(SELECT_USER);
        select_builder.push(" WHERE TRUE");
        Self::push_filters(&mut select_builder, request);
        select_builder
            .push(format!(
                " ORDER BY {} {}",
                sort_column(&request.sort_by),
                sort_order(&request.order)
            ))
            .push(" LIMIT ")
            .push_bind(request.per_page)
            .push(" OFFSET ")
            .push_bind(request.offset());

        let rows = select_builder.build().fetch_all(self.db.pool()).await?;

        let mut users = Vec::with_capacity(rows.len());
        for row in &rows {
            let mut user = Self::map_user(row)?;
            user.roles = self.get_user_roles(user.id).await?;
            users.push(user);
        }

        Ok((users, total_count))
    }

    async fn create(&self, tx: &mut PgConnection, user: &CreateUser) -> Result<Uuid, DomainError> {
        let row = sqlx::query(
            "INSERT INTO iam.users \
             (user_name, avatar_id, email, password, first_name, last_name, status_id, department_id) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING id",
        )
        .bind(&user.user_name)
        .bind(&user.avatar_id)
        .bind(&user.email)
        .bind(&user.password)
        .bind(&user.first_name)
        .bind(&user.last_name)
        .bind(user.status)
        .bind(user.department_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|err| {
            if pg_error::is(&err, pg_error::UNIQUE_VIOLATION) {
                DomainError::UserDuplicateConstraint
            } else if pg_error::is(&err, pg_error::FOREIGN_KEY_VIOLATION) {
                DomainError::DepartmentNotFoundConstraint
            } else {
                DomainError::Database(err)
            }
        })?;

        let user_id: Uuid = row.try_get("id")?;

        self.add_user_roles(tx, user_id, &user.roles).await?;

        Ok(user_id)
    }

    async fn find_by_id(&self, user_id: Uuid) -> Result<Option<User>, DomainError> {
        let row = sqlx::query(&format!("{SELECT_USER} WHERE u.id = $1"))
            .bind(user_id)
            .fetch_optional(self.db.pool())
            .await?;

        self.hydrate(row).await
    }

    async fn find_by_user_name(&self, user_name: &str) -> Result<Option<User>, DomainError> {
        let row = sqlx::query(&format!("{SELECT_USER} WHERE u.user_name = $1"))
            .bind(user_name)
            .fetch_optional(self.db.pool())
            .await?;

        self.hydrate(row).await
    }

    async fn find_by_email(&self, email: &str) -> Result<Option<User>, DomainError> {
        let row = sqlx::query(&format!("{SELECT_USER} WHERE u.email = $1"))
            .bind(email)
            .fetch_optional(self.db.pool())
            .await?;

        self.hydrate(row).await
    }

    async fn add_user_roles(
        &self,
        tx: &mut PgConnection,
        user_id: Uuid,
        roles: &[String],
    ) -> Result<(), DomainError> {
        if roles.is_empty() {
            return Ok(());
        }

        // `ON CONFLICT` keeps re-assigning an existing role idempotent.
        sqlx::query(
            "INSERT INTO iam.user_has_roles (user_id, role_id) \
             SELECT $1, r.id FROM iam.roles r WHERE r.name = ANY($2) \
             ON CONFLICT (user_id, role_id) DO NOTHING",
        )
        .bind(user_id)
        .bind(roles)
        .execute(tx)
        .await?;

        Ok(())
    }

    async fn remove_user_roles(
        &self,
        tx: &mut PgConnection,
        user_id: Uuid,
        roles: &[String],
    ) -> Result<(), DomainError> {
        if roles.is_empty() {
            return Ok(());
        }

        sqlx::query(
            "DELETE FROM iam.user_has_roles \
             WHERE user_id = $1 \
             AND role_id IN (SELECT id FROM iam.roles WHERE name = ANY($2))",
        )
        .bind(user_id)
        .bind(roles)
        .execute(tx)
        .await?;

        Ok(())
    }

    async fn partial_update(
        &self,
        tx: &mut PgConnection,
        user_id: Uuid,
        fields: &UpdateUser,
    ) -> Result<(), DomainError> {
        if fields.has_no_column_changes() {
            return Ok(());
        }

        let mut builder = QueryBuilder::new("UPDATE iam.users SET ");
        let mut separated = builder.separated(", ");

        if let Some(avatar_id) = &fields.avatar_id {
            separated
                .push("avatar_id = ")
                .push_bind_unseparated(avatar_id);
        }
        if let Some(first_name) = &fields.first_name {
            separated
                .push("first_name = ")
                .push_bind_unseparated(first_name);
        }
        if let Some(last_name) = &fields.last_name {
            separated
                .push("last_name = ")
                .push_bind_unseparated(last_name);
        }
        if let Some(email) = &fields.email {
            separated.push("email = ").push_bind_unseparated(email);
        }
        if let Some(department_id) = fields.department_id {
            separated
                .push("department_id = ")
                .push_bind_unseparated(department_id);
        }
        if let Some(status) = &fields.status {
            let status_id = UserStatus::id_of(status).ok_or(DomainError::StatusNotFound)?;
            separated
                .push("status_id = ")
                .push_bind_unseparated(status_id);
        }
        if let Some(password) = &fields.password {
            separated
                .push("password = ")
                .push_bind_unseparated(password);
        }

        builder.push(" WHERE id = ").push_bind(user_id);

        builder.build().execute(tx).await.map_err(|err| {
            match pg_error::code_of(&err).as_deref() {
                Some(pg_error::NOT_NULL_VIOLATION) => DomainError::StatusNotFound,
                Some(pg_error::FOREIGN_KEY_VIOLATION) => DomainError::DepartmentNotFoundConstraint,
                Some(pg_error::UNIQUE_VIOLATION) => DomainError::UserDuplicateConstraint,
                _ => DomainError::Database(err),
            }
        })?;

        Ok(())
    }

    /// Soft delete: the row is kept and flipped to the `Deleted` status.
    async fn delete(&self, tx: &mut PgConnection, user_id: Uuid) -> Result<(), DomainError> {
        sqlx::query("UPDATE iam.users SET status_id = $1 WHERE id = $2")
            .bind(DELETED_USER_STATUS)
            .bind(user_id)
            .execute(tx)
            .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_whitelisted_columns_reach_the_order_by_clause() {
        assert_eq!(sort_column("email"), "u.email");
        assert_eq!(sort_column("department"), "d.name");
        assert_eq!(sort_column("status"), "us.status");
        // Anything unknown, including an injection attempt, falls back.
        assert_eq!(
            sort_column("u.password; DROP TABLE iam.users"),
            "u.user_name"
        );
        assert_eq!(sort_column(""), "u.user_name");
    }

    #[test]
    fn the_sort_direction_is_never_taken_verbatim() {
        assert_eq!(sort_order("asc"), "asc");
        assert_eq!(sort_order("desc"), "desc");
        assert_eq!(sort_order("; DELETE FROM iam.users"), "desc");
    }

    #[test]
    fn known_filters_become_bound_parameters() {
        let request = ListRequest::from_query(
            "status=Active&department_id=1&role=Admin&first_name=App&nonsense=1",
        );

        let mut builder = QueryBuilder::<sqlx::Postgres>::new(COUNT_USER);
        builder.push(" WHERE TRUE");
        PgUserRepository::push_filters(&mut builder, &request);

        let sql = builder.sql();

        assert!(sql.contains("us.id = $1"));
        assert!(sql.contains("u.department_id = $2"));
        assert!(sql.contains("r.name = $3"));
        assert!(sql.contains("u.first_name ILIKE $4"));
        // No value is ever interpolated into the statement.
        assert!(!sql.contains("Active"));
        assert!(!sql.contains("nonsense"));
    }

    #[test]
    fn an_unknown_status_name_is_dropped_rather_than_bound() {
        let request = ListRequest::from_query("status=Imaginary");

        let mut builder = QueryBuilder::<sqlx::Postgres>::new(COUNT_USER);
        builder.push(" WHERE TRUE");
        PgUserRepository::push_filters(&mut builder, &request);

        assert!(!builder.sql().contains("us.id ="));
    }
}
