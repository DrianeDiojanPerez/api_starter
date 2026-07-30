mod auth;
mod db;
mod jwt;
mod logger;
mod mail;
mod server;

pub use auth::Auth;
pub use db::Db;
pub use jwt::Jwt;
pub use logger::{LogLevel, Logger};
pub use mail::Mail;
pub use server::{Deployment, Environment, Server};

use crate::package::env;

pub type ConfigError = env::Error;

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
            logger: Self::load_logger()?,
            deployment: Self::load_deployment()?,
            db: Self::load_db()?,
            mail: Self::load_mail()?,
            jwt: Self::load_jwt()?,
            auth: Self::load_auth()?,
        })
    }

    fn load_server() -> Result<Server, ConfigError> {
        Ok(Server {
            port: env::u16_or("APP_PORT", 3000)?,
        })
    }

    fn load_logger() -> Result<Logger, ConfigError> {
        Ok(Logger {
            level: env::variant_or_default("LOGGER_LEVEL")?,
            directory: env::string_or("LOGGER_DIRECTORY", "storage/logs"),
        })
    }

    fn load_deployment() -> Result<Deployment, ConfigError> {
        Ok(Deployment {
            name: env::string_or("APP_NAME", "App_sample"),
            environment: env::variant_or_default("APP_ENVIRONMENT")?,
            time_zone: env::string_or("APP_TIMEZONE", "America/Belize"),
        })
    }

    fn load_db() -> Result<Db, ConfigError> {
        Ok(Db {
            host: env::string_or("DB_HOST", "127.0.0.1"),
            port: env::u16_or("DB_PORT", 5432)?,
            database: env::string_or("DB_DATABASE", "api_starter"),
            username: env::string_or("DB_USERNAME", "root"),
            password: env::string_or("DB_PASSWORD", "password"),
            max_connections: env::u32_or("DB_MAX_CONNECTIONS", 10)?,
            run_migrations: env::boolean_or("DB_RUN_MIGRATIONS", true)?,
        })
    }

    fn load_mail() -> Result<Mail, ConfigError> {
        Ok(Mail {
            host: env::string_or("MAIL_HOST", "mail"),
            port: env::u16_or("MAIL_PORT", 1025)?,
            username: env::string("MAIL_USERNAME").unwrap_or_default(),
            password: env::string("MAIL_PASSWORD").unwrap_or_default(),
            from_address: env::string_or("MAIL_FROM_ADDRESS", "noreply@example.com"),
            from_name: env::string_or("MAIL_FROM_NAME", "noreply"),
        })
    }

    fn load_jwt() -> Result<Jwt, ConfigError> {
        Ok(Jwt::new(env::required("JWT_SECRET")?))
    }

    fn load_auth() -> Result<Auth, ConfigError> {
        Ok(Auth {
            access_token_ttl_in_seconds: env::i64_or(
                "AUTHENTICATION_ACCESS_TOKEN_TTL_SECONDS",
                3600,
            )?,
            refresh_token_ttl_in_seconds: env::i64_or(
                "AUTHENTICATION_REFRESH_TOKEN_TTL_SECONDS",
                604_800,
            )?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_a_dsn_from_the_database_settings() {
        let db = Db {
            host: "database".to_owned(),
            port: 5432,
            database: "api_starter".to_owned(),
            username: "postgres".to_owned(),
            password: "password".to_owned(),
            max_connections: 10,
            run_migrations: true,
        };

        assert_eq!(
            db.to_url(),
            "postgres://postgres:password@database:5432/api_starter"
        );
    }

    #[test]
    fn escapes_the_characters_that_would_break_the_dsn() {
        let db = Db {
            host: "database".to_owned(),
            port: 5432,
            database: "api_starter".to_owned(),
            username: "user@corp".to_owned(),
            password: "p@ss:w/rd?#".to_owned(),
            max_connections: 10,
            run_migrations: true,
        };

        assert_eq!(
            db.to_url(),
            "postgres://user%40corp:p%40ss%3Aw%2Frd%3F%23@database:5432/api_starter"
        );
    }

    #[test]
    fn the_database_password_stays_out_of_the_debug_output() {
        let db = Db {
            host: "database".to_owned(),
            port: 5432,
            database: "api_starter".to_owned(),
            username: "postgres".to_owned(),
            password: "hunter2".to_owned(),
            max_connections: 10,
            run_migrations: true,
        };

        assert!(!format!("{db:?}").contains("hunter2"));
    }

    #[test]
    fn the_logger_level_is_a_name_not_a_number() {
        let logger = |value: &str| Logger {
            level: value.parse().expect("the level should parse"),
            directory: "storage/logs".to_owned(),
        };

        assert_eq!(logger("debug").level.to_string(), "debug");
        assert_eq!(logger("ERROR").level.to_string(), "error");
        // A number is not a level.
        assert!("0".parse::<LogLevel>().is_err());
    }

    #[test]
    fn only_production_disables_the_stdout_logger() {
        let deployment = |environment: Environment| Deployment {
            name: "App_sample".to_owned(),
            environment,
            time_zone: "America/Belize".to_owned(),
        };

        assert!(deployment(Environment::Production).is_production());
        assert!(!deployment(Environment::Development).is_production());
        assert!(!deployment(Environment::Local).is_production());
        // Only the three names are accepted.
        assert!("staging".parse::<Environment>().is_err());
    }

    #[test]
    fn the_jwt_secret_stays_out_of_the_debug_and_json_output() {
        let jwt = Jwt::new("super-secret");

        assert!(!format!("{jwt:?}").contains("super-secret"));
        assert_eq!(jwt.secret().expose(), b"super-secret");
    }
}
