// SPDX-License-Identifier: AGPL-3.0-or-later
//! The job queue. Heavy BIM work (diffs, merges, IFC export, imports) runs in engine workers, which claim
//! jobs over the internal API. A job with a cache key is computed once: results are content-addressed.
use crate::{access::Project, app::AppState, error::ApiResult};
use serde::Serialize;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct JobState {
    pub id: Uuid,
    pub kind: String,
    pub status: String,
    pub progress: f32,
    pub result: Option<Value>,
    pub error: Option<String>,
}

impl JobState {
    pub fn done(&self) -> bool {
        self.status == "done"
    }
    pub fn failed(&self) -> bool {
        self.status == "failed"
    }
}

const COLUMNS: &str = "id, kind, status, progress, result, error";

/// Queue a job. With a cache key, an existing job is returned instead (a failed one is queued again).
pub async fn enqueue(db: &PgPool, project: Uuid, kind: &str, args: Value, cache_key: Option<&str>) -> ApiResult<JobState> {
    let id = Uuid::new_v4();
    let row: JobState = sqlx::query_as(&format!(
        "INSERT INTO jobs (id, project_id, kind, args, cache_key) VALUES ($1, $2, $3, $4, $5) \
         ON CONFLICT (cache_key) DO UPDATE SET \
           status = CASE WHEN jobs.status = 'failed' THEN 'queued' ELSE jobs.status END, \
           error = CASE WHEN jobs.status = 'failed' THEN NULL ELSE jobs.error END, \
           args = CASE WHEN jobs.status = 'failed' THEN EXCLUDED.args ELSE jobs.args END \
         RETURNING {COLUMNS}"
    ))
    .bind(id)
    .bind(project)
    .bind(kind)
    .bind(args)
    .bind(cache_key)
    .fetch_one(db)
    .await?;
    Ok(row)
}

pub async fn get(db: &PgPool, id: Uuid) -> ApiResult<Option<(JobState, Uuid)>> {
    let row: Option<(Uuid, String, String, f32, Option<Value>, Option<String>, Uuid)> =
        sqlx::query_as("SELECT id, kind, status, progress, result, error, project_id FROM jobs WHERE id = $1")
            .bind(id)
            .fetch_optional(db)
            .await?;
    Ok(row.map(|(id, kind, status, progress, result, error, p)| (JobState { id, kind, status, progress, result, error }, p)))
}

/// Cache keys are namespaced by project: results are never shared between customers (spec section 7).
pub fn key(project: &Project, kind: &str, parts: &[&str]) -> String {
    format!("{}:{kind}:{}", project.id, parts.join(":"))
}

pub async fn enqueue_summary(state: &AppState, project: &Project, commit: &str) -> ApiResult<JobState> {
    enqueue(&state.db, project.id, "summary", json!({"rev": commit}), Some(&key(project, "summary", &[commit]))).await
}

fn is_commit_id(s: &str) -> bool {
    matches!(s.len(), 40 | 64) && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// What a job needs from its project's repository, the least that lets it work. History jobs (merges,
/// element history, whole-project exports) need everything; a name instead of a commit id does too.
pub fn fetch_plan(kind: &str, args: &Value) -> Value {
    let wanted: Option<(Vec<&str>, u32)> = match kind {
        "export_ifc" | "checks" => args["rev"].as_str().map(|r| (vec![r], 1)),
        "summary" => args["rev"].as_str().map(|r| (vec![r], 2)),
        "compare" | "settings_diff" => args["from"].as_str().zip(args["to"].as_str()).map(|(a, b)| (if a == b { vec![a] } else { vec![a, b] }, 1)),
        "import_kim" => return json!({"mode": "none"}),
        _ => None,
    };
    match wanted {
        Some((commits, depth)) if commits.iter().all(|c| is_commit_id(c)) => json!({"mode": "commits", "commits": commits, "depth": depth}),
        _ => json!({"mode": "full"}),
    }
}

/// Wait for a job to finish, polling the database (used where the API answers synchronously, e.g. Combine).
pub async fn wait(db: &PgPool, id: Uuid, timeout: std::time::Duration) -> ApiResult<JobState> {
    let start = std::time::Instant::now();
    loop {
        let (job, _) = get(db, id).await?.ok_or_else(crate::error::AppError::not_found)?;
        if job.done() || job.failed() || start.elapsed() > timeout {
            return Ok(job);
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
}
