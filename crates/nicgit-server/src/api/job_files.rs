// SPDX-License-Identifier: AGPL-3.0-or-later
//! The files one engine job may read and write, over HTTP: workers mount no shared storage (decision N30).
//! Paths come from the job, so every one is checked here; a job can only name files of its own project.
use crate::{
    api::internal::{authorize_job, RunningJob},
    app::AppState,
    error::{ApiResult, AppError},
    repos,
};
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use futures::TryStreamExt;
use serde_json::json;
use tokio::io::AsyncWriteExt;
use tokio_util::io::ReaderStream;
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/internal/jobs/{id}/upload", get(upload))
        .route("/internal/jobs/{id}/artifacts/{*path}", get(read_artifact).put(write_artifact))
}

fn may_write(kind: &str, path: &str) -> bool {
    match kind {
        "export_ifc" => path.starts_with("ifc/") && path.ends_with(".ifc"),
        "export_kim" => path.starts_with("exports/") && path.ends_with(".kim"),
        _ => false,
    }
}

fn may_read(kind: &str, path: &str) -> bool {
    kind == "checks" && path.starts_with("ifc/") && path.ends_with(".ifc")
}

fn produces_artifacts(kind: &str) -> bool {
    matches!(kind, "export_ifc" | "checks" | "export_kim")
}

fn forbidden() -> AppError {
    AppError::new(StatusCode::FORBIDDEN, "forbidden", "this job cannot use that file")
}

/// The most a job may store in one file: what a project may hold, and so also what an import may unpack to.
fn artifact_limit(state: &AppState) -> u64 {
    crate::limits::bytes(state.cfg.max_repo_mb)
}

fn too_large() -> AppError {
    AppError::new(StatusCode::PAYLOAD_TOO_LARGE, "too_large", "the file is too large")
}

async fn read_artifact(State(state): State<AppState>, headers: HeaderMap, Path((id, path)): Path<(Uuid, String)>) -> ApiResult<Response> {
    let job = authorize_job(&state, &headers, id).await?;
    if !produces_artifacts(&job.kind) {
        return Err(forbidden());
    }
    let file = repos::artifact_path(&state.cfg, job.project, &path).ok_or_else(|| AppError::bad_request("not an artifact path"))?;
    if !may_read(&job.kind, &path) {
        return Err(forbidden());
    }
    stream(&file).await
}

async fn stream(path: &std::path::Path) -> ApiResult<Response> {
    let file = tokio::fs::File::open(path).await.map_err(|_| AppError::not_found())?;
    let len = file.metadata().await?.len();
    Ok(([(header::CONTENT_LENGTH, len.to_string())], Body::from_stream(ReaderStream::new(file))).into_response())
}

async fn write_artifact(State(state): State<AppState>, headers: HeaderMap, Path((id, path)): Path<(Uuid, String)>, body: Body) -> ApiResult<Json<serde_json::Value>> {
    let job = authorize_job(&state, &headers, id).await?;
    if !produces_artifacts(&job.kind) {
        return Err(forbidden());
    }
    let target = repos::artifact_path(&state.cfg, job.project, &path).ok_or_else(|| AppError::bad_request("not an artifact path"))?;
    if !may_write(&job.kind, &path) {
        return Err(AppError::bad_request("not an artifact path"));
    }
    let dir = target.parent().ok_or_else(|| AppError::bad_request("not an artifact path"))?;
    tokio::fs::create_dir_all(dir).await?;
    // A partial file never appears under the real name.
    let tmp = dir.join(format!(".part-{}", Uuid::new_v4()));
    let written = save(&state, &tmp, body).await;
    let renamed = match written {
        Ok(()) => tokio::fs::rename(&tmp, &target).await.map_err(AppError::from),
        Err(e) => Err(e),
    };
    if let Err(e) = renamed {
        let _ = tokio::fs::remove_file(&tmp).await;
        return Err(e);
    }
    let cfg = state.cfg.clone();
    let project = job.project;
    tokio::task::spawn_blocking(move || crate::limits::trim_cache(&cfg, project, &target)).await.map_err(AppError::internal)?;
    Ok(Json(json!({"ok": true})))
}

async fn save(state: &AppState, tmp: &std::path::Path, body: Body) -> ApiResult<()> {
    let limit = artifact_limit(state);
    let mut file = tokio::fs::File::create(tmp).await?;
    let mut stream = body.into_data_stream();
    let mut total = 0u64;
    while let Some(chunk) = stream.try_next().await.map_err(|e| AppError::bad_request(e.to_string()))? {
        total += chunk.len() as u64;
        if total > limit {
            return Err(too_large());
        }
        file.write_all(&chunk).await?;
    }
    file.flush().await?;
    Ok(())
}

fn is_import(job: &RunningJob) -> bool {
    matches!(job.kind.as_str(), "import_kim" | "import_ifc")
}

async fn upload(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<Uuid>) -> ApiResult<Response> {
    let job = authorize_job(&state, &headers, id).await?;
    if !is_import(&job) {
        return Err(forbidden());
    }
    let name = job.args["upload"].as_str().unwrap_or_default();
    let path = repos::upload_path(&state.cfg, job.project, name).ok_or_else(|| AppError::bad_request("not an upload name"))?;
    stream(&path).await
}
