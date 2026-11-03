// SPDX-License-Identifier: AGPL-3.0-or-later
//! Whole projects in and out: import a .kim or .ifc as a new project; download a .kim.
use crate::{
    access,
    api::{jobs::respond, projects::{create_project, NewProject}},
    app::AppState,
    auth::{CurrentUser, MaybeUser},
    error::{ApiResult, AppError},
    gitread::Git,
    jobs,
};
use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Multipart, Path, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use nicgit_core::roles::Role;
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;
use tokio_util::io::ReaderStream;
use uuid::Uuid;

pub fn router(max_upload_mb: u64) -> Router<AppState> {
    let limit = usize::try_from((max_upload_mb + 1) << 20).unwrap_or(usize::MAX);
    Router::new()
        .route("/api/projects/import", post(import).layer(DefaultBodyLimit::max(limit)))
        .route("/api/projects/{owner}/{slug}/export.kim", get(export))
}

fn too_large(mb: u64) -> AppError {
    AppError::new(StatusCode::PAYLOAD_TOO_LARGE, "too_large", format!("files are limited to {mb} MB"))
}

async fn import(State(state): State<AppState>, user: CurrentUser, mut form: Multipart) -> ApiResult<Json<serde_json::Value>> {
    let max = state.cfg.max_upload_mb << 20;
    let (mut owner, mut slug) = (String::new(), String::new());
    let mut saved: Option<(std::path::PathBuf, String, String)> = None; // (path, kind, original name)
    let result: ApiResult<()> = async {
        while let Some(mut field) = form.next_field().await.map_err(|e| {
            if e.status() == StatusCode::PAYLOAD_TOO_LARGE { too_large(state.cfg.max_upload_mb) } else { AppError::bad_request(e.to_string()) }
        })? {
            match field.name() {
                Some("owner") => owner = field.text().await.map_err(|e| AppError::bad_request(e.to_string()))?,
                Some("slug") => slug = field.text().await.map_err(|e| AppError::bad_request(e.to_string()))?,
                Some("file") => {
                    let original = field.file_name().unwrap_or("upload").to_string();
                    let lower = original.to_lowercase();
                    let kind = if lower.ends_with(".kim") {
                        "kim"
                    } else if lower.ends_with(".ifc") {
                        "ifc"
                    } else {
                        return Err(AppError::bad_request("upload a .kim or .ifc file"));
                    };
                    let name = format!("{}.{kind}", Uuid::new_v4());
                    let path = state.cfg.data_dir.join("uploads").join(&name);
                    saved = Some((path.clone(), kind.to_string(), original));
                    let mut file = tokio::fs::File::create(&path).await?;
                    let mut written = 0u64;
                    while let Some(chunk) = field.chunk().await.map_err(|e| {
                        if e.status() == StatusCode::PAYLOAD_TOO_LARGE { too_large(state.cfg.max_upload_mb) } else { AppError::bad_request(e.to_string()) }
                    })? {
                        written += chunk.len() as u64;
                        if written > max {
                            return Err(too_large(state.cfg.max_upload_mb));
                        }
                        file.write_all(&chunk).await?;
                    }
                    file.flush().await?;
                }
                _ => {}
            }
        }
        Ok(())
    }
    .await;
    let cleanup = |saved: &Option<(std::path::PathBuf, String, String)>| {
        if let Some((p, _, _)) = saved {
            let _ = std::fs::remove_file(p);
        }
    };
    if let Err(e) = result {
        cleanup(&saved);
        return Err(e);
    }
    let Some((path, kind, original)) = saved.clone() else {
        return Err(AppError::bad_request("attach a .kim or .ifc file"));
    };
    let project = match create_project(&state, &user, &NewProject { owner, slug, description: String::new(), visibility: "private".into() }).await {
        Ok(p) => p,
        Err(e) => {
            cleanup(&saved);
            return Err(e);
        }
    };
    let (name, email): (String, String) = sqlx::query_as("SELECT a.display_name, u.email FROM users u JOIN accounts a ON a.id = u.account_id WHERE u.account_id = $1")
        .bind(user.id)
        .fetch_one(&state.db)
        .await?;
    let upload = path.file_name().unwrap().to_string_lossy().to_string();
    let job = if kind == "kim" {
        jobs::enqueue(&state.db, project.id, "import_kim", json!({"upload": upload, "max_bytes": max.saturating_mul(20), "by": user.id}), None).await?
    } else {
        jobs::enqueue(
            &state.db,
            project.id,
            "import_ifc",
            json!({"upload": upload, "option": "main", "message": format!("Imported from {original}"), "author": [name, email], "by": user.id}),
            None,
        )
        .await?
    };
    crate::audit::record(&state.db, Some(project.owner_id), Some(user.id), "project.import", &project.full_name(), json!({"kind": kind, "file": original})).await?;
    Ok(Json(json!({"project": project.to_json(Some(Role::Admin)), "job": job.id})))
}

#[derive(Deserialize)]
struct ExportQuery {
    scope: Option<String>,
    option: Option<String>,
}

async fn export(
    State(state): State<AppState>,
    MaybeUser(user): MaybeUser,
    Path((owner, slug)): Path<(String, String)>,
    Query(q): Query<ExportQuery>,
) -> ApiResult<Response> {
    let (p, _) = access::load(&state, &owner, &slug, user.as_ref().map(|u| u.id), Role::Viewer).await?;
    let scope = q.scope.unwrap_or_else(|| "full".into());
    if scope != "full" && scope != "latest" {
        return Err(AppError::bad_request("scope is full or latest"));
    }
    let option = q.option.unwrap_or_else(|| p.default_option.clone());
    // The export is keyed by what it contains: every ref for a full export, the option's head otherwise.
    let state_key = if scope == "full" {
        let refs = crate::git::refs(&state.cfg.git, &p.repo_dir(&state).join(".git")).await.map_err(AppError::internal)?;
        hex::encode(Sha256::digest(serde_json::to_vec(&refs).unwrap_or_default()))
    } else {
        Git::new(&p.repo_dir(&state)).resolve(&option).await.unwrap_or_default()
    };
    let job = jobs::enqueue(
        &state.db,
        p.id,
        "export_kim",
        json!({"scope": scope, "option": option, "name": p.slug}),
        Some(&jobs::key(&p, "export_kim", &[&scope, &option, &state_key])),
    )
    .await?;
    if !job.done() {
        return Ok(respond(job));
    }
    let path = std::path::PathBuf::from(job.result.as_ref().and_then(|r| r["path"].as_str()).unwrap_or_default());
    let cache = state.cfg.data_dir.join("cache").join(p.id.to_string());
    let inside = path.canonicalize().ok().zip(cache.canonicalize().ok()).is_some_and(|(f, c)| f.starts_with(c));
    if !inside {
        sqlx::query("UPDATE jobs SET status = 'failed', error = 'cache missing' WHERE id = $1").bind(job.id).execute(&state.db).await?;
        return Err(AppError::new(StatusCode::GONE, "expired", "the export expired; request it again"));
    }
    let file = tokio::fs::File::open(&path).await?;
    let len = file.metadata().await?.len();
    if let Some(u) = &user {
        crate::audit::record(&state.db, Some(p.owner_id), Some(u.id), "project.export", &p.full_name(), json!({"scope": scope})).await?;
        crate::metering::record(&state, p.owner_id, Some(p.id), "egress_bytes", len as i64).await?;
    }
    Ok((
        [
            (header::CONTENT_TYPE, "application/zip".to_string()),
            (header::CONTENT_LENGTH, len.to_string()),
            (header::CONTENT_DISPOSITION, format!("attachment; filename=\"{}.kim\"", p.slug)),
        ],
        Body::from_stream(ReaderStream::new(file)),
    )
        .into_response())
}
