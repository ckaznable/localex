use anyhow::Result;
use tracing::info;
use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    EnvFilter,
    prelude::*,
};
use tracing_appender::rolling::{RollingFileAppender, Rotation};

pub fn init_logger<T: AsRef<str>>(filename: T, use_tui: bool) -> Result<()> {
    let file_appender = RollingFileAppender::new(
        Rotation::NEVER,
        dirs::cache_dir().unwrap(),
        filename.as_ref(),
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

    if !use_tui {
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
