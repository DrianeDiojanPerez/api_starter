use api_starter::config::AppConfig;
use api_starter::provider::Provider;
use api_starter::server;
use api_starter::shared::logger;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = AppConfig::load()?;

    // The guard flushes the file appender, so it lives for the whole process.
    let _logger_guard = logger::init(&config.logger, &config.deployment);

    tracing::info!(
        environment = %config.deployment.environment,
        port = config.server.port,
        "starting api starter"
    );

    let provider = Provider::inject_default(config).await?;

    server::serve(provider).await
}
