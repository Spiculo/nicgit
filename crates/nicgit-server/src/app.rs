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

/// The two faces of the server: what people use, and what engine workers use (decision N35).
pub struct Routers {
    pub public: Router,
    pub internal: Router,
}

pub async fn build(cfg: Arc<Config>) -> anyhow::Result<(Routers, AppState)> {
    let db = crate::db::connect(&cfg.database_url).await?;
    std::fs::create_dir_all(cfg.data_dir.join("repos"))?;
    std::fs::create_dir_all(cfg.data_dir.join("cache"))?;
    // Anything here was being received when the server last stopped, and nobody is waiting for it now.
    let _ = crate::repos::remove_all(&crate::repos::incoming_dir(&cfg));
    std::fs::create_dir_all(crate::repos::incoming_dir(&cfg))?;
    crate::api::google::sync_site_admin(&db, cfg.site_admin_email.as_deref()).await?;
    let state = AppState { db, cfg };
    let mut public = Router::new()
        .route("/api/health", get(health))
        .merge(crate::api::router(state.cfg.max_file_mb))
        .merge(crate::git::http::router())
        .route("/api/{*rest}", axum::routing::any(api_not_found))
        .with_state(state.clone());
    if let Some(dir) = &state.cfg.web_dir {
        // The web app: static files, and index.html for every other path (client-side routes).
        let index = dir.join("index.html");
        public = public.fallback_service(ServeDir::new(dir).fallback(ServeFile::new(index)));
    }
    let internal = crate::api::internal_router().merge(crate::git::http::job_router()).with_state(state.clone());
    Ok((Routers { public, internal }, state))
}

async fn health() -> Json<Value> {
    Json(json!({"ok": true, "version": env!("CARGO_PKG_VERSION")}))
}

async fn api_not_found() -> crate::error::AppError {
    crate::error::AppError::not_found()
}
