use std::sync::Arc;
use std::time::Duration;

pub mod migrations;

use sqlx::migrate::MigrateError;
use sqlx::postgres::{PgPoolOptions, PgQueryResult};
use sqlx::{PgPool, Postgres, Transaction};

use crate::config;

#[derive(Clone)]
pub struct Database {
    pool: PgPool,
}

impl Database {
    pub async fn connect(cfg: &config::Db) -> Result<Self, sqlx::Error> {
        let pool = PgPoolOptions::new()
            .max_connections(cfg.max_connections)
            .acquire_timeout(Duration::from_secs(5))
            .connect(&cfg.to_url())
            .await?;

        Ok(Self { pool })
    }

    pub fn from_pool(pool: PgPool) -> Self {
        Self { pool }
    }

    /// sqlx takes an advisory lock first, so several replicas starting at
    /// once is safe.
    pub async fn migrate(&self) -> Result<(), MigrateError> {
        for module in migrations::all() {
            self.migrate_with(&module).await?;
        }

        Ok(())
    }

    pub async fn migrate_module(&self, name: &str) -> Result<bool, MigrateError> {
        match migrations::find(name) {
            Some(module) => self.migrate_with(&module).await.map(|()| true),
            None => Ok(false),
        }
    }

    async fn migrate_with(
        &self,
        module: &migrations::ModuleMigrations,
    ) -> Result<(), MigrateError> {
        tracing::info!(module = module.name, "applying migrations");

        module.migrator.run(&self.pool).await
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

#[derive(Clone)]
pub struct TxManager {
    db: Arc<Database>,
}

impl TxManager {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    pub async fn begin(&self) -> Result<Transaction<'static, Postgres>, sqlx::Error> {
        self.db.pool().begin().await
    }
}

pub mod pg_error {
    pub const UNIQUE_VIOLATION: &str = "23505";
    pub const NOT_NULL_VIOLATION: &str = "23502";
    pub const FOREIGN_KEY_VIOLATION: &str = "23503";

    pub fn code_of(err: &sqlx::Error) -> Option<String> {
        match err {
            sqlx::Error::Database(db_err) => db_err.code().map(|code| code.into_owned()),
            _ => None,
        }
    }

    pub fn constraint_of(err: &sqlx::Error) -> Option<String> {
        match err {
            sqlx::Error::Database(db_err) => db_err.constraint().map(str::to_owned),
            _ => None,
        }
    }

    pub fn is(err: &sqlx::Error, code: &str) -> bool {
        code_of(err).is_some_and(|actual| actual == code)
    }
}

pub type QueryResult = PgQueryResult;
