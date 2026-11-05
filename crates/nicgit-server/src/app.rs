// SPDX-License-Identifier: AGPL-3.0-or-later
use crate::config::Config;
use axum::{routing::get, Json, Router};
use tower_http::services::{ServeDir, ServeFile};
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
    crate::api::google::sync_site_admin(&db, cfg.site_admin_email.as_deref()).await?;
    let state = AppState { db, cfg };
    let mut router = Router::new()
        .route("/api/health", get(health))
        .merge(crate::api::router(state.cfg.max_upload_mb))
        .merge(crate::git::http::router())
        .route("/api/{*rest}", axum::routing::any(api_not_found))
        .with_state(state.clone());
    if let Some(dir) = &state.cfg.web_dir {
        // The web app: static files, and index.html for every other path (client-side routes).
        let index = dir.join("index.html");
        router = router.fallback_service(ServeDir::new(dir).fallback(ServeFile::new(index)));
    }
    Ok((router, state))
}

async fn health() -> Json<Value> {
    Json(json!({"ok": true, "version": env!("CARGO_PKG_VERSION")}))
}

async fn api_not_found() -> crate::error::AppError {
    crate::error::AppError::not_found()
}
