use std::path::PathBuf;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Initialize tracing with both stdout and file output.
/// Logs are written to `%APPDATA%\FlowLens\flowlens.log`.
pub fn init() {
    let log_dir = std::env::var("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("FlowLens");
    let _ = std::fs::create_dir_all(&log_dir);
    let log_path = log_dir.join("flowlens.log");

    let file_appender = tracing_appender::rolling::daily(&log_dir, "flowlens.log");

    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,flowlens_tauri=debug,flowlens_core=debug,flowlens_capture=debug,flowlens_tls=debug,flowlens_protocol=debug"));

    let _ = tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stdout))
        .with(tracing_subscriber::fmt::layer().with_writer(file_appender).with_ansi(false))
        .try_init();

    tracing::info!(path = %log_path.display(), "logging initialized");
}