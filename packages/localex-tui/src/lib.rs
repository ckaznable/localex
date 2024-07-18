#![allow(unused_must_use)]

use anyhow::Result;

mod app;
mod components;

pub mod config;
pub mod cli;

pub async fn main(param: config::Config) -> Result<()> {
    logger::init_logger(logger::LoggerConfig {
        filename: &param.log_file_name,
        stdout: false,
    })?;

    let mut tui = app::App::new().await?;
    tui.run().await
}
