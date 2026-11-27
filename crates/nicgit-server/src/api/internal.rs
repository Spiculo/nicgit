// SPDX-License-Identifier: AGPL-3.0-or-later
//! The internal API of engine workers. The pool token only claims jobs. Each claim returns a token for that
//! one job, which is all a job can use: it reaches its own project's files over HTTP and nothing else, and
//! it stops working when the job finishes or is handed out again (decision N30).
use crate::{
    app::AppState,
    error::{ApiResult, AppError},
};
use axum::{
    extract::{FromRequestParts, Path, State},
    http::{request::Parts, HeaderMap, StatusCode},
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

/// A job as seen by the requests made under its token.
#[derive(Debug, Clone)]
pub struct RunningJob {
    pub id: Uuid,
    pub project: Uuid,
    pub kind: String,
    pub args: Value,
}

impl RunningJob {
    /// The refs this job may push, for the pre-receive hook, and the options protected while it does.
    /// None: the job only reads.
    pub fn push_rule(&self) -> Option<(String, Vec<String>)> {
        let exact = |option: &str| Some((format!("exact:refs/heads/{option}"), vec![option.to_string()]));
        match self.kind.as_str() {
            "combine" => exact(self.args["target"].as_str()?),
            "import_ifc" => exact(self.args["option"].as_str().unwrap_or("main")),
            "import_nic" => Some(("create".to_string(), vec![])),
            _ => None,
        }
    }
}

/// The job a request's bearer token belongs to. Unknown, finished and reclaimed all look the same.
pub async fn authorize_job(state: &AppState, headers: &HeaderMap, id: Uuid) -> Result<RunningJob, AppError> {
    let token = headers.get(axum::http::header::AUTHORIZATION).and_then(|v| v.to_str().ok()).and_then(|h| h.strip_prefix("Bearer ")).map(str::trim);
    let Some(token) = token.filter(|t| !t.is_empty()) else { return Err(AppError::unauthorized()) };
    let row: Option<(Uuid, String, Value)> = sqlx::query_as("SELECT project_id, kind, args FROM jobs WHERE id = $1 AND token_hash = $2 AND status = 'running'")
        .bind(id)
        .bind(crate::auth::sha256_hex(token))
        .fetch_optional(&state.db)
        .await?;
    let (project, kind, args) = row.ok_or_else(AppError::unauthorized)?;
    Ok(RunningJob { id, project, kind, args })
}

/// A request from an engine worker's supervisor (header `X-Worker-Token`): it may only claim jobs.
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

/// A running job whose worker has not reported for this long is assumed lost and handed out again.
/// Workers report every minute (nicgit_worker's heartbeat), however long the job takes.
const STALE_MINUTES: i32 = 10;

/// A job that took its worker down this many times would keep doing so: it fails instead of going round again.
const MAX_ATTEMPTS: i32 = 3;

/// Give up on jobs that keep losing their worker. The uploads they were importing go with them.
async fn fail_repeatedly_lost(state: &AppState) -> ApiResult<()> {
    let lost: Vec<(Uuid, Value)> = sqlx::query_as(
        "UPDATE jobs SET status = 'failed', error = 'the engine stopped repeatedly while working on this', progress = 0, finished_at = now(), token_hash = NULL \
         WHERE status = 'running' AND attempts >= $1 AND coalesce(heartbeat_at, started_at) < now() - make_interval(mins => $2) \
         RETURNING project_id, args",
    )
    .bind(MAX_ATTEMPTS)
    .bind(STALE_MINUTES)
    .fetch_all(&state.db)
    .await?;
    for (project, args) in lost {
        if let Some(path) = args["upload"].as_str().and_then(|n| crate::repos::upload_path(&state.cfg, project, n)) {
            let _ = tokio::fs::remove_file(path).await;
        }
    }
    Ok(())
}

async fn claim(State(state): State<AppState>, _w: Worker, Json(b): Json<Claim>) -> ApiResult<Response> {
    fail_repeatedly_lost(&state).await?;
    let token = crate::auth::new_session_token();
    // The account with the fewest jobs running goes first, oldest job first within that, so one account filling
    // the queue cannot keep everyone else waiting (decision N33).
    let row: Option<(Uuid, Uuid, String, Value)> = sqlx::query_as(
        "UPDATE jobs SET status = 'running', started_at = now(), heartbeat_at = now(), attempts = attempts + 1, progress = 0, token_hash = $3 \
         WHERE id = (SELECT j.id FROM jobs j JOIN projects p ON p.id = j.project_id WHERE j.kind = ANY($1) AND \
                       (j.status = 'queued' OR (j.status = 'running' AND coalesce(j.heartbeat_at, j.started_at) < now() - make_interval(mins => $2))) \
                     ORDER BY (SELECT count(*) FROM jobs r JOIN projects rp ON rp.id = r.project_id WHERE r.status = 'running' AND rp.owner_id = p.owner_id), j.created_at \
                     FOR UPDATE OF j SKIP LOCKED LIMIT 1) \
         RETURNING id, project_id, kind, args",
    )
    .bind(&b.kinds)
    .bind(STALE_MINUTES)
    .bind(crate::auth::sha256_hex(&token))
    .fetch_optional(&state.db)
    .await?;
    let Some((id, project, kind, args)) = row else { return Ok(StatusCode::NO_CONTENT.into_response()) };
    let fetch = crate::jobs::fetch_plan(&kind, &args);
    Ok(Json(json!({"id": id, "project": project, "kind": kind, "args": args, "token": token, "fetch": fetch})).into_response())
}

#[derive(Deserialize)]
struct Progress {
    progress: f32,
}

async fn progress(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<Uuid>, Json(b): Json<Progress>) -> ApiResult<Json<Value>> {
    authorize_job(&state, &headers, id).await?;
    sqlx::query("UPDATE jobs SET progress = $2, heartbeat_at = now() WHERE id = $1 AND status = 'running'")
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

/// A combine's claim about what it did, checked against the repository.
async fn verify_combine(state: &AppState, job: &RunningJob, result: Option<&Value>) -> Result<(), AppError> {
    let Some(target) = job.args["target"].as_str() else { return Ok(()) };
    let head = crate::git::run(&state.cfg.git, &crate::repos::git_dir(&state.cfg, job.project), &["rev-parse", "--verify", "-q", &format!("refs/heads/{target}")]).await.ok();
    let claimed = result.and_then(|r| r["commit"].as_str());
    if head.as_deref().map(str::trim) == claimed && claimed.is_some() {
        return Ok(());
    }
    Err(AppError::bad_request("the engine's result does not match the design option's head"))
}

async fn finish(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<Uuid>, Json(mut b): Json<Finish>) -> ApiResult<Json<Value>> {
    let job = authorize_job(&state, &headers, id).await?;
    if job.kind == "combine" && b.error.is_none() {
        if let Err(e) = verify_combine(&state, &job, b.result.as_ref()).await {
            b.error = Some(e.message);
            b.result = None;
        }
    }
    let (status, progress) = if b.error.is_some() { ("failed", 0.0f32) } else { ("done", 1.0) };
    let row: Option<(Uuid, String, Value)> = sqlx::query_as(
        "UPDATE jobs SET status = $2, result = $3, error = $4, progress = $5, finished_at = now(), token_hash = NULL \
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
    // An upload is used once, whatever became of the import.
    if matches!(kind.as_str(), "import_nic" | "import_ifc") {
        if let Some(path) = args["upload"].as_str().and_then(|n| crate::repos::upload_path(&state.cfg, project, n)) {
            let _ = tokio::fs::remove_file(path).await;
        }
    }
    crate::events::on_job_finished(&state, project, id, &kind, &args, b.result.as_ref(), b.error.as_deref()).await?;
    Ok(Json(json!({"ok": true})))
}
