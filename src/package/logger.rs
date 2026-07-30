use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

use crate::config::{Deployment, Logger as LoggerConfig};

/// Keeps the non blocking appender worker alive. Dropping it flushes and stops
/// the background writer, so `main` must hold on to it.
pub struct LoggerGuard(#[allow(dead_code)] WorkerGuard);

pub fn init(cfg: &LoggerConfig, deployment: &Deployment) -> LoggerGuard {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(format!("api_starter={0},tower_http={0}", cfg.level)));

    let file_appender = tracing_appender::rolling::Builder::new()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix("log")
        .filename_suffix("log")
        .build(&cfg.directory)
        .expect("failed to create the log directory");

    let (file_writer, guard) = tracing_appender::non_blocking(file_appender);

    let file_layer = tracing_subscriber::fmt::layer()
        .json()
        .with_current_span(true)
        .with_span_list(false)
        .with_writer(file_writer);

    let stdout_layer = (!deployment.is_production()).then(|| {
        tracing_subscriber::fmt::layer()
            .json()
            .with_current_span(true)
            .with_span_list(false)
            .with_writer(std::io::stdout)
            .boxed()
    });

    tracing_subscriber::registry()
        .with(filter)
        .with(file_layer)
        .with(stdout_layer)
        .init();

    LoggerGuard(guard)
}
