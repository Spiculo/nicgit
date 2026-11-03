// SPDX-License-Identifier: AGPL-3.0-or-later
use crate::config::Config;
use axum::{routing::get, Json, Router};
use serde_json::{json, Value};
use sqlx::PgPool;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub cfg: Arc<Config>,
}

pub async fn build(cfg: Arc<Config>) -> anyhow::Result<(Router, AppState)> {
    let db = crate::db::connect(&cfg.database_url).await?;
    std::fs::create_dir_all(cfg.data_dir.join("repos"))?;
    std::fs::create_dir_all(cfg.data_dir.join("cache"))?;
    std::fs::create_dir_all(cfg.data_dir.join("uploads"))?;
    let state = AppState { db, cfg };
    let router = Router::new().route("/api/health", get(health)).merge(crate::api::router()).with_state(state.clone());
    Ok((router, state))
}

async fn health() -> Json<Value> {
    Json(json!({"ok": true, "version": env!("CARGO_PKG_VERSION")}))
}
