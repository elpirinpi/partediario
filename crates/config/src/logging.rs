use parte_diario_core::paths;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;

pub fn init_logging() -> Result<WorkerGuard, String> {
    let dir = paths::logs_dir();
    paths::ensure_dir(&dir)?;
    let appender = tracing_appender::rolling::daily(&dir, "parte-diario.log");
    let (writer, guard) = tracing_appender::non_blocking(appender);
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(writer)
        .with_ansi(false)
        .with_target(true)
        .init();
    Ok(guard)
}
