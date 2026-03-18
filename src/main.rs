#![allow(dead_code)]

mod action;
mod app;
mod components;
mod config;
mod event;
mod keys;
mod panels;
mod services;
mod theme;
mod tui;

use app::App;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let mut app = App::new();
    app.run().await?;

    Ok(())
}
