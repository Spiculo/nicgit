// SPDX-License-Identifier: AGPL-3.0-or-later
//! Git smart HTTP: `git upload-pack` / `git receive-pack --stateless-rpc` behind NicGit's auth.
use crate::{
    access::{self, Project},
    app::AppState,
    auth::MaybeUser,
    error::{ApiResult, AppError},
};
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use futures::TryStreamExt;
use nicgit_core::roles::Role;
use serde::Deserialize;
use std::process::Stdio;
use tokio::io::AsyncWriteExt;
use tokio_util::io::{ReaderStream, StreamReader};
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/{owner}/{repo}/info/refs", get(info_refs))
        .route("/{owner}/{repo}/git-upload-pack", post(upload_pack))
        .route("/{owner}/{repo}/git-receive-pack", post(receive_pack))
}

#[derive(Clone, Copy, PartialEq)]
enum Service {
    Upload,
    Receive,
}

impl Service {
    fn name(self) -> &'static str {
        match self {
            Service::Upload => "git-upload-pack",
            Service::Receive => "git-receive-pack",
        }
    }
    fn need(self) -> Role {
        match self {
            Service::Upload => Role::Viewer,
            Service::Receive => Role::Member,
        }
    }
}

fn challenge() -> Response {
    let mut r = (StatusCode::UNAUTHORIZED, "NicGit: sign in with your user name and a personal access token\n").into_response();
    r.headers_mut().insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Basic realm=\"NicGit\""));
    r
}

/// Resolve the project and check the role. Anonymous callers get a Basic challenge so git asks for a
/// token; signed-in callers without access get 404.
async fn authorize(state: &AppState, user: &MaybeUser, owner: &str, repo: &str, service: Service) -> Result<(Project, Role), Response> {
    let slug = repo.strip_suffix(".git").ok_or_else(|| AppError::not_found().into_response())?;
    let user_id = user.0.as_ref().map(|u| u.id);
    match access::load(state, owner, slug, user_id, service.need()).await {
        Ok(ok) => Ok(ok),
        Err(e) if user_id.is_none() && (e.status == StatusCode::NOT_FOUND || e.status == StatusCode::FORBIDDEN) => Err(challenge()),
        Err(e) => Err(e.into_response()),
    }
}

fn pkt_line(s: &str) -> Vec<u8> {
    let mut v = format!("{:04x}", s.len() + 4).into_bytes();
    v.extend_from_slice(s.as_bytes());
    v
}

#[derive(Deserialize)]
struct ServiceQuery {
    service: String,
}

async fn info_refs(
    State(state): State<AppState>,
    user: MaybeUser,
    Path((owner, repo)): Path<(String, String)>,
    Query(q): Query<ServiceQuery>,
    headers: HeaderMap,
) -> Response {
    let service = match q.service.as_str() {
        "git-upload-pack" => Service::Upload,
        "git-receive-pack" => Service::Receive,
        _ => return AppError::bad_request("dumb HTTP is not supported").into_response(),
    };
    let (project, _) = match authorize(&state, &user, &owner, &repo, service).await {
        Ok(ok) => ok,
        Err(r) => return r,
    };
    let mut cmd = tokio::process::Command::new(&state.cfg.git);
    cmd.arg(&service.name()[4..]).args(["--stateless-rpc", "--advertise-refs"]).arg(project.repo_dir(&state).join(".git"));
    if let Some(p) = headers.get("git-protocol").and_then(|v| v.to_str().ok()) {
        cmd.env("GIT_PROTOCOL", p);
    }
    let out = match cmd.output().await {
        Ok(o) if o.status.success() => o,
        Ok(o) => return AppError::internal(String::from_utf8_lossy(&o.stderr)).into_response(),
        Err(e) => return AppError::internal(e).into_response(),
    };
    let mut body = pkt_line(&format!("# service={}\n", service.name()));
    body.extend_from_slice(b"0000");
    body.extend_from_slice(&out.stdout);
    (
        [
            (header::CONTENT_TYPE, format!("application/x-{}-advertisement", service.name())),
            (header::CACHE_CONTROL, "no-cache".to_string()),
        ],
        body,
    )
        .into_response()
}

async fn upload_pack(State(state): State<AppState>, user: MaybeUser, Path((owner, repo)): Path<(String, String)>, headers: HeaderMap, body: Body) -> Response {
    rpc(state, user, owner, repo, Service::Upload, headers, body).await.unwrap_or_else(|e| e)
}

async fn receive_pack(State(state): State<AppState>, user: MaybeUser, Path((owner, repo)): Path<(String, String)>, headers: HeaderMap, body: Body) -> Response {
    rpc(state, user, owner, repo, Service::Receive, headers, body).await.unwrap_or_else(|e| e)
}

async fn rpc(
    state: AppState,
    user: MaybeUser,
    owner: String,
    repo: String,
    service: Service,
    headers: HeaderMap,
    body: Body,
) -> Result<Response, Response> {
    let (project, role) = authorize(&state, &user, &owner, &repo, service).await?;
    let git_dir = project.repo_dir(&state).join(".git");
    let mut cmd = tokio::process::Command::new(&state.cfg.git);
    cmd.arg(&service.name()[4..]).arg("--stateless-rpc").arg(&git_dir);
    cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::inherit()).kill_on_drop(true);
    if let Some(p) = headers.get("git-protocol").and_then(|v| v.to_str().ok()) {
        cmd.env("GIT_PROTOCOL", p);
    }
    let before = if service == Service::Receive {
        let protected = crate::rules::protected_options(&state.db, project.id).await.map_err(|e| e.into_response())?;
        cmd.env("NICGIT_ROLE", role.as_str())
            .env("NICGIT_PROTECTED", protected.join("\u{1f}"))
            .env("NICGIT_GIT", &state.cfg.git);
        Some(super::refs(&state.cfg.git, &git_dir).await.map_err(|e| AppError::internal(e).into_response())?)
    } else {
        None
    };
    let mut child = cmd.spawn().map_err(|e| AppError::internal(e).into_response())?;
    let mut stdin = child.stdin.take().expect("piped");
    let stdout = child.stdout.take().expect("piped");

    let gzip = headers.get(header::CONTENT_ENCODING).is_some_and(|v| v.as_bytes() == b"gzip");
    let reader = StreamReader::new(body.into_data_stream().map_err(std::io::Error::other));
    tokio::spawn(async move {
        let result = if gzip {
            let mut dec = async_compression::tokio::bufread::GzipDecoder::new(reader);
            tokio::io::copy(&mut dec, &mut stdin).await
        } else {
            let mut reader = reader;
            tokio::io::copy(&mut reader, &mut stdin).await
        };
        if let Err(e) = result {
            tracing::warn!("git request body: {e}");
        }
        let _ = stdin.shutdown().await;
    });

    // After the process ends, record what the push changed.
    if let Some(before) = before {
        let user_id = user.0.as_ref().map(|u| u.id);
        let state2 = state.clone();
        let project2 = project.clone();
        tokio::spawn(async move {
            match child.wait().await {
                Ok(_) => after_push(&state2, &project2, user_id, before, &git_dir).await,
                Err(e) => tracing::warn!("receive-pack: {e}"),
            }
        });
    } else {
        tokio::spawn(async move {
            let _ = child.wait().await;
        });
    }
    let content_type = format!("application/x-{}-result", service.name());
    Ok(([(header::CONTENT_TYPE, content_type), (header::CACHE_CONTROL, "no-cache".to_string())], Body::from_stream(ReaderStream::new(stdout)))
        .into_response())
}

async fn after_push(state: &AppState, project: &Project, user: Option<Uuid>, before: super::RefMap, git_dir: &std::path::Path) {
    let after = match super::refs(&state.cfg.git, git_dir).await {
        Ok(r) => r,
        Err(e) => return tracing::warn!("listing refs after a push: {e}"),
    };
    let updates = super::changed(&before, &after);
    if updates.is_empty() {
        return;
    }
    if let Err(e) = crate::events::on_push(state, project, user, &updates).await {
        tracing::error!("after push to {}: {}", project.full_name(), e.message);
    }
}

pub type _Unused = ApiResult<()>;
