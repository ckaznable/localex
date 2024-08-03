use anyhow::Result;
use tracing::info;
use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    EnvFilter,
    prelude::*,
};
use tracing_appender::rolling::{RollingFileAppender, Rotation};

#[cfg(target_os = "android")]
use tracing_subscriber::prelude::*;

#[derive(Default)]
pub struct LoggerConfig<'a> {
    pub filename: &'a str,
    pub stdout: bool,
}

#[cfg(target_os = "android")]
pub fn init_logger() {
    tracing_subscriber::registry()
        .with(tracing_android_trace::AndroidTraceLayer::new())
        .try_init()
        .unwrap();
}

#[cfg(not(target_os = "android"))]
pub fn init_logger(config: LoggerConfig) -> Result<()> {
    let mut log_dir = dirs::cache_dir().unwrap();
    log_dir.push("localex");
    let file_appender = RollingFileAppender::new(
        Rotation::NEVER,
        log_dir,
        config.filename,
    );

    let file_layer = fmt::layer()
        .with_writer(file_appender)
        .with_ansi(true)
        .with_span_events(FmtSpan::CLOSE)
        .event_format(fmt::format().compact());

    let subscriber = tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("info")))
        .with(file_layer);

    if config.stdout {
        let stdout_layer = fmt::layer()
            .with_writer(std::io::stdout)
            .with_ansi(true)
            .with_span_events(FmtSpan::CLOSE)
            .event_format(fmt::format().compact());

        subscriber.with(stdout_layer).init();
    } else {
        subscriber.init();
    }

    info!("Logger initialized successfully");
    Ok(())
}
