mod auth;
mod db;
mod jwt;
mod logger;
mod mail;
mod server;

pub use auth::Auth;
pub use db::Db;
pub use jwt::Jwt;
pub use logger::Logger;
pub use mail::Mail;
pub use server::{Deployment, Server};

use std::env;
use std::str::FromStr;

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("environment variable `{key}` is required")]
    Missing { key: &'static str },
    #[error("environment variable `{key}` has an invalid value: {value}")]
    Invalid { key: &'static str, value: String },
}

/// Aggregated application configuration, loaded once at start up and shared
/// with every handler through the provider.
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub server: Server,
    pub logger: Logger,
    pub deployment: Deployment,
    pub db: Db,
    pub mail: Mail,
    pub jwt: Jwt,
    pub auth: Auth,
}

impl AppConfig {
    pub fn load() -> Result<Self, ConfigError> {
        if dotenvy::dotenv().is_err() {
            eprintln!("No .env file found. Using default environment values");
        }

        Ok(Self {
            server: Self::load_server()?,
            logger: Self::load_logger(),
            deployment: Self::load_deployment(),
            db: Self::load_db()?,
            mail: Self::load_mail()?,
            jwt: Self::load_jwt()?,
            auth: Self::load_auth()?,
        })
    }

    fn load_server() -> Result<Server, ConfigError> {
        Ok(Server {
            port: parsed("APP_PORT")?.unwrap_or(3000),
        })
    }

    fn load_logger() -> Logger {
        Logger {
            level: string("LOGGER_LEVEL").unwrap_or_else(|| "0".to_owned()),
            directory: string("LOGGER_DIRECTORY").unwrap_or_else(|| "storage/logs".to_owned()),
        }
    }

    fn load_deployment() -> Deployment {
        Deployment {
            name: string("APP_NAME").unwrap_or_else(|| "App_sample".to_owned()),
            environment: string("APP_ENVIRONMENT").unwrap_or_else(|| "DEVELOPMENT".to_owned()),
            time_zone: string("APP_TIMEZONE").unwrap_or_else(|| "America/Belize".to_owned()),
        }
    }

    fn load_db() -> Result<Db, ConfigError> {
        Ok(Db {
            host: string("DB_HOST").unwrap_or_else(|| "127.0.0.1".to_owned()),
            port: parsed("DB_PORT")?.unwrap_or(5432),
            database: string("DB_DATABASE").unwrap_or_else(|| "api_starter".to_owned()),
            username: string("DB_USERNAME").unwrap_or_else(|| "root".to_owned()),
            password: string("DB_PASSWORD").unwrap_or_else(|| "password".to_owned()),
            max_connections: parsed("DB_MAX_CONNECTIONS")?.unwrap_or(10),
        })
    }

    fn load_mail() -> Result<Mail, ConfigError> {
        Ok(Mail {
            host: string("MAIL_HOST").unwrap_or_else(|| "mail".to_owned()),
            port: parsed("MAIL_PORT")?.unwrap_or(1025),
            username: string("MAIL_USERNAME").unwrap_or_default(),
            password: string("MAIL_PASSWORD").unwrap_or_default(),
            from_address: string("MAIL_FROM_ADDRESS")
                .unwrap_or_else(|| "noreply@example.com".to_owned()),
            from_name: string("MAIL_FROM_NAME").unwrap_or_else(|| "noreply".to_owned()),
        })
    }

    fn load_jwt() -> Result<Jwt, ConfigError> {
        let secret = string("JWT_SECRET").ok_or(ConfigError::Missing { key: "JWT_SECRET" })?;
        Ok(Jwt::new(secret))
    }

    fn load_auth() -> Result<Auth, ConfigError> {
        Ok(Auth {
            access_token_ttl_in_seconds: parsed("AUTHENTICATION_ACCESS_TOKEN_TTL_SECONDS")?
                .unwrap_or(3600),
            refresh_token_ttl_in_seconds: parsed("AUTHENTICATION_REFRESH_TOKEN_TTL_SECONDS")?
                .unwrap_or(604_800),
        })
    }
}

fn string(key: &'static str) -> Option<String> {
    match env::var(key) {
        Ok(value) if !value.trim().is_empty() => Some(value.trim().to_owned()),
        _ => None,
    }
}

fn parsed<T: FromStr>(key: &'static str) -> Result<Option<T>, ConfigError> {
    match string(key) {
        None => Ok(None),
        Some(value) => value
            .parse::<T>()
            .map(Some)
            .map_err(|_| ConfigError::Invalid { key, value }),
    }
}
