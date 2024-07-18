#![allow(unused_must_use)]

use anyhow::Result;

mod app;
mod components;

pub mod config;
pub mod cli;

pub async fn main() -> Result<()> {
    let mut tui = app::App::new()?;
    tui.run().await
}
