mod config;
mod controllers;
mod models;
mod net;
mod proto;
mod server;
mod views;

use anyhow::Result;
use config::Config;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    server::run(Config::from_env()?).await
}
