use api_starter::config::AppConfig;
use api_starter::database::{migrations, Database};
use api_starter::provider::Provider;
use api_starter::server;
use api_starter::shared::logger;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = AppConfig::load()?;

    // The guard flushes the file appender, so it lives for the whole process.
    let _logger_guard = logger::init(&config.logger, &config.deployment);

    let args: Vec<String> = std::env::args().skip(1).collect();

    match args.first().map(String::as_str) {
        None => serve(config).await,
        Some("migrate") => migrate(config, args.get(1).map(String::as_str)).await,
        Some(other) => {
            anyhow::bail!("unknown command `{other}`. Usage: server [migrate [<module>]]")
        }
    }
}

async fn serve(config: AppConfig) -> anyhow::Result<()> {
    tracing::info!(
        environment = %config.deployment.environment,
        port = config.server.port,
        "starting api starter"
    );

    let provider = Provider::inject_default(config).await?;

    server::serve(provider).await
}

/// `server migrate [module]`. Applies the migrations this binary carries, in
/// registry order, then exits. Lets a deployment migrate without shipping
/// sqlx-cli or the SQL files.
async fn migrate(config: AppConfig, module: Option<&str>) -> anyhow::Result<()> {
    let database = Database::connect(&config.db).await?;

    match module {
        None => database.migrate().await?,
        Some(name) if database.migrate_module(name).await? => {}
        Some(name) => anyhow::bail!(
            "unknown module `{name}`. Known modules: {}",
            migrations::names().join(", ")
        ),
    }

    tracing::info!("migrations are up to date");

    Ok(())
}
