// SPDX-License-Identifier: AGPL-3.0-or-later
use nicgit_server::{app, config::Config};
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();
    let cfg = Arc::new(Config::from_env()?);
    let (router, _state) = app::build(cfg.clone()).await?;
    let listener = tokio::net::TcpListener::bind(&cfg.bind).await?;
    tracing::info!("listening on {}", listener.local_addr()?);
    axum::serve(listener, router).await?;
    Ok(())
}
