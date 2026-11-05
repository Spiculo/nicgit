// SPDX-License-Identifier: AGPL-3.0-or-later
use nicgit_server::{app, config::Config};
use std::sync::Arc;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("hook") && args.get(2).map(String::as_str) == Some("pre-receive") {
        std::process::exit(nicgit_server::git::hook::pre_receive());
    }
    if args.get(1).map(String::as_str) == Some("dev-user") {
        return tokio::runtime::Runtime::new()?.block_on(dev_user(&args[2..]));
    }
    tokio::runtime::Runtime::new()?.block_on(serve())
}

/// `nicgit-server dev-user --slug S --email E [--name N] [--session]`: an account for development and tests,
/// made without Google. Prints an access token, or with --session a session cookie value. Needs NICGIT_DEV=1
/// and the database credentials; it is never reachable over the network.
async fn dev_user(args: &[String]) -> anyhow::Result<()> {
    anyhow::ensure!(std::env::var("NICGIT_DEV").is_ok_and(|v| v == "1"), "dev-user is for development: set NICGIT_DEV=1");
    let value = |flag: &str| args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned();
    let slug = value("--slug").ok_or_else(|| anyhow::anyhow!("--slug is required"))?;
    let email = value("--email").ok_or_else(|| anyhow::anyhow!("--email is required"))?;
    let name = value("--name").unwrap_or_else(|| slug.clone());
    let db = nicgit_server::db::connect(&Config::database_url_from_env()).await?;
    let id = nicgit_server::dev::create_user(&db, &slug, &email, &name).await?;
    let secret = if args.iter().any(|a| a == "--session") {
        nicgit_server::dev::new_session(&db, id).await?
    } else {
        nicgit_server::dev::new_token(&db, id, "dev-user").await?
    };
    println!("{secret}");
    Ok(())
}

async fn serve() -> anyhow::Result<()> {
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
