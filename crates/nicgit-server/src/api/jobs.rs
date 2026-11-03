// SPDX-License-Identifier: AGPL-3.0-or-later
//! Job status for the web app, and the shared shape of job-backed answers.
use crate::{
    access,
    app::AppState,
    auth::MaybeUser,
    error::{ApiResult, AppError},
    jobs::{self, JobState},
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use nicgit_core::roles::Role;
use serde_json::{json, Value};
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new().route("/api/jobs/{id}", get(get_job))
}

async fn get_job(State(state): State<AppState>, MaybeUser(user): MaybeUser, Path(id): Path<Uuid>) -> ApiResult<Json<Value>> {
    let (job, project) = jobs::get(&state.db, id).await?.ok_or_else(AppError::not_found)?;
    let p: access::Project = sqlx::query_as(&format!(
        "SELECT {} FROM projects p JOIN accounts a ON a.id = p.owner_id WHERE p.id = $1",
        access::PROJECT_COLUMNS
    ))
    .bind(project)
    .fetch_one(&state.db)
    .await?;
    let role = access::project_role(&state, user.map(|u| u.id), &p).await?;
    if role.is_none_or(|r| r < Role::Viewer) {
        return Err(AppError::not_found());
    }
    Ok(Json(json!(job)))
}

/// Answer with a job's result when it is done (200), its error when it failed (422), or a reference to
/// poll while it runs (202 `{job, status, progress}`).
pub fn respond(job: JobState) -> Response {
    if job.done() {
        return Json(job.result.unwrap_or(Value::Null)).into_response();
    }
    if job.failed() {
        let msg = job.error.unwrap_or_else(|| "the job failed".into());
        return (StatusCode::UNPROCESSABLE_ENTITY, Json(json!({"error": "job_failed", "message": msg, "job": job.id}))).into_response();
    }
    (StatusCode::ACCEPTED, Json(json!({"job": job.id, "status": job.status, "progress": job.progress}))).into_response()
}
