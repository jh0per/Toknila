//! Daemon log files.

use crate::paths::Paths;

/// Sends logs to `<home>/logs/daemon.log.<date>`, rotated daily, seven files kept.
/// Keep the returned guard alive for the life of the process.
pub fn init_logging(paths: &Paths) -> Result<tracing_appender::non_blocking::WorkerGuard, String> {
    let appender = tracing_appender::rolling::Builder::new()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix("daemon")
        .filename_suffix("log")
        .max_log_files(7)
        .build(paths.logs())
        .map_err(|e| format!("cannot open log directory: {e}"))?;
    let (writer, guard) = tracing_appender::non_blocking(appender);
    let filter = tracing_subscriber::EnvFilter::try_from_env("TOKNILA_LOG")
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(writer)
        .with_ansi(false)
        .try_init()
        .ok();
    Ok(guard)
}
