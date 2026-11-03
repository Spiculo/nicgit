// SPDX-License-Identifier: AGPL-3.0-or-later
//! Views computed by the engine: compare, settings diff, element history and a version's IFC.
use crate::{
    api::{jobs::respond, versions::load},
    app::AppState,
    auth::MaybeUser,
    error::{ApiResult, AppError},
    jobs,
};
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::header,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use serde::Deserialize;
use serde_json::json;
use tokio_util::io::ReaderStream;

pub fn router() -> Router<AppState> {
    let base = "/api/projects/{owner}/{slug}";
    Router::new()
        .route(&format!("{base}/compare"), get(compare))
        .route(&format!("{base}/settings-diff"), get(settings_diff))
        .route(&format!("{base}/elements/{{guid}}/history"), get(history))
        .route(&format!("{base}/versions/{{rev}}/model.ifc"), get(model_ifc))
        .route(&format!("{base}/versions/{{rev}}/checks"), get(checks))
}

/// Queue (or find) an engine job for a view. Anonymous visitors of public projects only get answers that
/// already exist: they cannot make the engine compute arbitrary version pairs (hosted cost, decision N7).
async fn view_job(state: &AppState, user: &MaybeUser, project_id: uuid::Uuid, kind: &str, args: serde_json::Value, key: String) -> ApiResult<jobs::JobState> {
    if user.0.is_none() {
        let existing: Option<jobs::JobState> = sqlx::query_as("SELECT id, kind, status, progress, result, error FROM jobs WHERE cache_key = $1")
            .bind(&key)
            .fetch_optional(&state.db)
            .await?;
        return existing.ok_or_else(|| AppError::new(axum::http::StatusCode::UNAUTHORIZED, "sign_in", "sign in to compare versions"));
    }
    jobs::enqueue(&state.db, project_id, kind, args, Some(&key)).await
}

#[derive(Deserialize)]
struct Pair {
    from: String,
    to: String,
}

async fn compare(State(state): State<AppState>, user: MaybeUser, Path((owner, slug)): Path<(String, String)>, Query(q): Query<Pair>) -> ApiResult<Response> {
    let (p, _, git) = load(&state, &user, &owner, &slug).await?;
    let (a, b) = (git.resolve(&q.from).await?, git.resolve(&q.to).await?);
    let job = view_job(&state, &user, p.id, "compare", json!({"from": a, "to": b}), jobs::key(&p, "compare", &[&a, &b])).await?;
    Ok(respond(job))
}

async fn settings_diff(State(state): State<AppState>, user: MaybeUser, Path((owner, slug)): Path<(String, String)>, Query(q): Query<Pair>) -> ApiResult<Response> {
    let (p, _, git) = load(&state, &user, &owner, &slug).await?;
    let (a, b) = (git.resolve(&q.from).await?, git.resolve(&q.to).await?);
    let job = view_job(&state, &user, p.id, "settings_diff", json!({"from": a, "to": b}), jobs::key(&p, "settings_diff", &[&a, &b])).await?;
    Ok(respond(job))
}

#[derive(Deserialize)]
struct RevQuery {
    rev: Option<String>,
}

fn valid_guid(g: &str) -> bool {
    (1..=64).contains(&g.len()) && g.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
}

async fn history(
    State(state): State<AppState>,
    user: MaybeUser,
    Path((owner, slug, guid)): Path<(String, String, String)>,
    Query(q): Query<RevQuery>,
) -> ApiResult<Response> {
    let (p, _, git) = load(&state, &user, &owner, &slug).await?;
    if !valid_guid(&guid) {
        return Err(AppError::bad_request("not an element id"));
    }
    let rev = git.resolve(q.rev.as_deref().unwrap_or(&p.default_option)).await?;
    let job = view_job(&state, &user, p.id, "element_history", json!({"guid": guid, "rev": rev}), jobs::key(&p, "history", &[&guid, &rev])).await?;
    Ok(respond(job))
}

/// The version as an IFC file, exported once by the engine and cached by commit.
async fn model_ifc(
    State(state): State<AppState>,
    user: MaybeUser,
    Path((owner, slug, rev)): Path<(String, String, String)>,
) -> ApiResult<Response> {
    let (p, _, git) = load(&state, &user, &owner, &slug).await?;
    let commit = git.resolve(&rev).await?;
    let job = jobs::enqueue(&state.db, p.id, "export_ifc", json!({"rev": commit}), Some(&jobs::key(&p, "export_ifc", &[&commit]))).await?;
    if !job.done() {
        return Ok(respond(job));
    }
    let path = std::path::PathBuf::from(job.result.as_ref().and_then(|r| r["path"].as_str()).unwrap_or_default());
    let cache = state.cfg.data_dir.join("cache").join(p.id.to_string());
    let inside = path.canonicalize().ok().zip(cache.canonicalize().ok()).is_some_and(|(f, c)| f.starts_with(c));
    if !inside {
        // The cache was cleared (or the result is not ours): export again.
        sqlx::query("UPDATE jobs SET status = 'failed', error = 'cache missing' WHERE id = $1").bind(job.id).execute(&state.db).await?;
        let job = jobs::enqueue(&state.db, p.id, "export_ifc", json!({"rev": commit}), Some(&jobs::key(&p, "export_ifc", &[&commit]))).await?;
        return Ok(respond(job));
    }
    let file = tokio::fs::File::open(&path).await?;
    let len = file.metadata().await?.len();
    let name = format!("{}-{}.ifc", p.slug, &commit[..8]);
    Ok((
        [
            (header::CONTENT_TYPE, "application/x-step".to_string()),
            (header::CONTENT_LENGTH, len.to_string()),
            (header::CONTENT_DISPOSITION, format!("inline; filename=\"{name}\"")),
            (header::CACHE_CONTROL, "private, max-age=31536000, immutable".to_string()),
        ],
        Body::from_stream(ReaderStream::new(file)),
    )
        .into_response())
}

#[derive(Deserialize)]
struct ChecksQuery {
    names: String,
}

/// Run (or read) checks on a version: an Action, so members only, and stopped by a spent budget.
async fn checks(
    State(state): State<AppState>,
    user: crate::auth::CurrentUser,
    Path((owner, slug, rev)): Path<(String, String, String)>,
    Query(q): Query<ChecksQuery>,
) -> ApiResult<Response> {
    let (p, _) = crate::access::load(&state, &owner, &slug, Some(user.id), nicgit_core::roles::Role::Member).await?;
    let git = crate::gitread::Git::new(&p.repo_dir(&state));
    let commit = git.resolve(&rev).await?;
    let names: Vec<String> = q.names.split(',').map(str::trim).filter(|s| !s.is_empty()).map(str::to_string).collect();
    if names.is_empty() {
        return Err(AppError::bad_request("name at least one check"));
    }
    let job = crate::actions::run(&state, &p, &commit, &names).await?;
    Ok(respond(job))
}
