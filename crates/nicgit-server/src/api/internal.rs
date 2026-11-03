// SPDX-License-Identifier: AGPL-3.0-or-later
//! The internal API engine workers use to claim and finish jobs. Workers hold no database credentials:
//! they only see the job, the project's repository and its cache folder.
use crate::{
    app::AppState,
    error::{ApiResult, AppError},
};
use axum::{
    extract::{FromRequestParts, Path, State},
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/internal/jobs/claim", post(claim))
        .route("/internal/jobs/{id}/progress", post(progress))
        .route("/internal/jobs/{id}/finish", post(finish))
}

/// A request from an engine worker (header `X-Worker-Token`).
pub struct Worker;

impl FromRequestParts<AppState> for Worker {
    type Rejection = AppError;
    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let given = parts.headers.get("x-worker-token").and_then(|v| v.to_str().ok()).unwrap_or("");
        // Compare digests so the comparison takes the same time whatever the input.
        let ok = Sha256::digest(given.as_bytes()) == Sha256::digest(state.cfg.worker_token.as_bytes());
        if ok && !given.is_empty() {
            Ok(Worker)
        } else {
            Err(AppError::unauthorized())
        }
    }
}

#[derive(Deserialize)]
struct Claim {
    kinds: Vec<String>,
}

/// A job left running for this long is assumed lost (its worker died) and handed out again.
const STALE_MINUTES: i32 = 30;

async fn claim(State(state): State<AppState>, _w: Worker, Json(b): Json<Claim>) -> ApiResult<Response> {
    let row: Option<(Uuid, Uuid, String, Value)> = sqlx::query_as(
        "UPDATE jobs SET status = 'running', started_at = now(), attempts = attempts + 1, progress = 0 \
         WHERE id = (SELECT id FROM jobs WHERE kind = ANY($1) AND \
                       (status = 'queued' OR (status = 'running' AND started_at < now() - make_interval(mins => $2))) \
                     ORDER BY created_at FOR UPDATE SKIP LOCKED LIMIT 1) \
         RETURNING id, project_id, kind, args",
    )
    .bind(&b.kinds)
    .bind(STALE_MINUTES)
    .fetch_optional(&state.db)
    .await?;
    let Some((id, project, kind, args)) = row else { return Ok(StatusCode::NO_CONTENT.into_response()) };
    let repo = crate::repos::dir(&state.cfg, project);
    let cache = state.cfg.data_dir.join("cache").join(project.to_string());
    let uploads = state.cfg.data_dir.join("uploads");
    Ok(Json(json!({
        "id": id, "project": project, "kind": kind, "args": args,
        "repo": repo.to_string_lossy(), "cache_dir": cache.to_string_lossy(), "uploads_dir": uploads.to_string_lossy(),
    }))
    .into_response())
}

#[derive(Deserialize)]
struct Progress {
    progress: f32,
}

async fn progress(State(state): State<AppState>, _w: Worker, Path(id): Path<Uuid>, Json(b): Json<Progress>) -> ApiResult<Json<Value>> {
    sqlx::query("UPDATE jobs SET progress = $2 WHERE id = $1 AND status = 'running'")
        .bind(id)
        .bind(b.progress.clamp(0.0, 1.0))
        .execute(&state.db)
        .await?;
    Ok(Json(json!({"ok": true})))
}

#[derive(Deserialize)]
struct Finish {
    result: Option<Value>,
    error: Option<String>,
}

async fn finish(State(state): State<AppState>, _w: Worker, Path(id): Path<Uuid>, Json(b): Json<Finish>) -> ApiResult<Json<Value>> {
    let (status, progress) = if b.error.is_some() { ("failed", 0.0f32) } else { ("done", 1.0) };
    let row: Option<(Uuid, String, Value)> = sqlx::query_as(
        "UPDATE jobs SET status = $2, result = $3, error = $4, progress = $5, finished_at = now() \
         WHERE id = $1 AND status = 'running' RETURNING project_id, kind, args",
    )
    .bind(id)
    .bind(status)
    .bind(&b.result)
    .bind(&b.error)
    .bind(progress)
    .fetch_optional(&state.db)
    .await?;
    let (project, kind, args) = row.ok_or_else(AppError::not_found)?;
    crate::events::on_job_finished(&state, project, id, &kind, &args, b.result.as_ref(), b.error.as_deref()).await?;
    Ok(Json(json!({"ok": true})))
}
