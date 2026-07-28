use std::sync::Arc;

use crate::config::AppConfig;
use crate::database::{Database, TxManager};
use crate::module::iam;
use crate::shared::auth::{Auth, AuthService, PostgresAuthStore};
use crate::shared::emailer::{Emailer, SmtpEmailer};
use crate::shared::jwt::{HmacTokenGenerator, TokenGenerator};
use crate::shared::rbac::{Engine, PostgresRbacStore, RbacEngine};

/// Role that bypasses every permission check.
const SUPER_ROLE: &str = "Admin";

/// Single composition root. Everything is built once at start up and shared
/// with the router through cheap `Arc` clones.
#[derive(Clone)]
pub struct Provider {
    pub config: Arc<AppConfig>,
    pub database: Arc<Database>,
    pub tx: TxManager,
    pub jwt: Arc<dyn TokenGenerator>,
    pub mailer: Arc<dyn Emailer>,
    pub auth: Arc<dyn Auth>,
    pub rbac: Arc<dyn Engine>,
    pub iam: iam::Services,
}

impl Provider {
    pub async fn inject_default(config: AppConfig) -> anyhow::Result<Self> {
        let database = Arc::new(Database::connect(&config.db).await?);

        if config.db.run_migrations {
            database.migrate().await?;
        }

        let tx = TxManager::new(database.clone());

        let jwt: Arc<dyn TokenGenerator> = Arc::new(HmacTokenGenerator::new(config.jwt.secret()));
        let mailer: Arc<dyn Emailer> = Arc::new(SmtpEmailer::new(&config.mail));

        let auth: Arc<dyn Auth> = Arc::new(AuthService::new(
            jwt.clone(),
            Arc::new(PostgresAuthStore::new(database.clone())),
            mailer.clone(),
            config.auth.access_token_ttl_in_seconds,
            config.auth.refresh_token_ttl_in_seconds,
        ));

        let rbac: Arc<dyn Engine> = Arc::new(RbacEngine::new(
            SUPER_ROLE,
            Arc::new(PostgresRbacStore::new(database.clone())),
        ));

        let iam = iam::Services::new(database.clone(), tx.clone());

        Ok(Self {
            config: Arc::new(config),
            database,
            tx,
            jwt,
            mailer,
            auth,
            rbac,
            iam,
        })
    }
}
