// SPDX-License-Identifier: AGPL-3.0-or-later
//! Git smart HTTP: `git upload-pack` / `git receive-pack --stateless-rpc` behind NicGit's auth.
use crate::{
    access::{self, Project},
    api::internal::{authorize_job, RunningJob},
    app::AppState,
    auth::MaybeUser,
    error::{ApiResult, AppError},
    limits::Room,
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

/// The same transport for engine jobs, on the workers' address.
pub fn job_router() -> Router<AppState> {
    Router::new()
        .route("/internal/jobs/{id}/git/info/refs", get(job_info_refs))
        .route("/internal/jobs/{id}/git/git-upload-pack", post(job_upload_pack))
        .route("/internal/jobs/{id}/git/git-receive-pack", post(job_receive_pack))
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
    if service == Service::Receive {
        match crate::limits::room(&state, project.id, false).await {
            Ok(Room { full: Some(reason), .. }) => return refuse_push(&reason),
            Ok(_) => {}
            Err(e) => return e.into_response(),
        }
    }
    advertise(&state, &project.repo_dir(&state).join(".git"), service, &headers, false).await
}

/// Told this way, git shows the pusher the reason rather than an HTTP status.
fn refuse_push(reason: &str) -> Response {
    let mut body = pkt_line(&format!("# service={}\n", Service::Receive.name()));
    body.extend_from_slice(b"0000");
    body.extend_from_slice(&pkt_line(&format!("ERR NicGit: {reason}\n")));
    ([(header::CONTENT_TYPE, format!("application/x-{}-advertisement", Service::Receive.name())), (header::CACHE_CONTROL, "no-cache".to_string())], body).into_response()
}

/// A job fetches single versions by commit id, which plain upload-pack only allows for branch tips.
/// `max_input` is the largest pack git may accept in a push.
fn git_command(state: &AppState, service: Service, job: bool, max_input: Option<u64>) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new(&state.cfg.git);
    if job && service == Service::Upload {
        cmd.args(["-c", "uploadpack.allowReachableSHA1InWant=true"]);
    }
    if let Some(n) = max_input {
        cmd.arg("-c").arg(format!("receive.maxInputSize={n}"));
    }
    cmd.arg(&service.name()[4..]);
    cmd
}

async fn advertise(state: &AppState, git_dir: &std::path::Path, service: Service, headers: &HeaderMap, job: bool) -> Response {
    let mut cmd = git_command(state, service, job, None);
    cmd.args(["--stateless-rpc", "--advertise-refs"]).arg(git_dir);
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
    let max_input = if service == Service::Receive {
        let room = crate::limits::room(&state, project.id, false).await.map_err(|e| e.into_response())?;
        if let Some(reason) = room.full {
            return Err(crate::limits::storage_full(reason).into_response());
        }
        Some(room.bytes)
    } else {
        None
    };
    let mut cmd = git_command(&state, service, false, max_input);
    cmd.arg("--stateless-rpc").arg(&git_dir);
    let before = if service == Service::Receive {
        let protected = crate::rules::protected_options(&state.db, project.id).await.map_err(|e| e.into_response())?;
        cmd.env("NICGIT_ROLE", role.as_str())
            .env("NICGIT_PROTECTED", protected.join("\u{1f}"))
            .env("NICGIT_MAX_FILE_BYTES", crate::limits::bytes(state.cfg.max_file_mb).to_string())
            .env("NICGIT_GIT", &state.cfg.git);
        Some(super::refs(&state.cfg.git, &git_dir).await.map_err(|e| AppError::internal(e).into_response())?)
    } else {
        None
    };
    let (mut child, response) = start_rpc(cmd, service, &headers, body)?;

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
    Ok(response)
}

/// Start git, feed it the request body and stream its output back.
fn start_rpc(mut cmd: tokio::process::Command, service: Service, headers: &HeaderMap, body: Body) -> Result<(tokio::process::Child, Response), Response> {
    cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::inherit()).kill_on_drop(true);
    if let Some(p) = headers.get("git-protocol").and_then(|v| v.to_str().ok()) {
        cmd.env("GIT_PROTOCOL", p);
    }
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
    let content_type = format!("application/x-{}-result", service.name());
    let response = ([(header::CONTENT_TYPE, content_type), (header::CACHE_CONTROL, "no-cache".to_string())], Body::from_stream(ReaderStream::new(stdout))).into_response();
    Ok((child, response))
}

// ---- engine jobs: the same transport under a job's token, for its own project only ----

fn job_forbidden() -> Response {
    AppError::new(StatusCode::FORBIDDEN, "forbidden", "this job only reads").into_response()
}

async fn job_info_refs(State(state): State<AppState>, Path(id): Path<Uuid>, Query(q): Query<ServiceQuery>, headers: HeaderMap) -> Response {
    let job = match authorize_job(&state, &headers, id).await {
        Ok(j) => j,
        Err(e) => return e.into_response(),
    };
    let service = match q.service.as_str() {
        "git-upload-pack" => Service::Upload,
        "git-receive-pack" if job.push_rule().is_some() => Service::Receive,
        "git-receive-pack" => return job_forbidden(),
        _ => return AppError::bad_request("dumb HTTP is not supported").into_response(),
    };
    if service == Service::Receive {
        match job_push_room(&state, &job).await {
            Ok(Err(reason)) => return refuse_push(&reason),
            Ok(Ok(_)) => {}
            Err(e) => return e.into_response(),
        }
    }
    advertise(&state, &crate::repos::git_dir(&state.cfg, job.project), service, &headers, true).await
}

async fn job_upload_pack(State(state): State<AppState>, Path(id): Path<Uuid>, headers: HeaderMap, body: Body) -> Response {
    let job = match authorize_job(&state, &headers, id).await {
        Ok(j) => j,
        Err(e) => return e.into_response(),
    };
    let mut cmd = git_command(&state, Service::Upload, true, None);
    cmd.arg("--stateless-rpc").arg(crate::repos::git_dir(&state.cfg, job.project));
    finish_job_rpc(cmd, Service::Upload, &headers, body)
}

/// A combine adds little, and a full project must still be able to finish its proposals, so it always has this.
const COMBINE_ALLOWANCE: u64 = 64 << 20;

/// How large a pack this job may push, or why it may not push.
async fn job_push_room(state: &AppState, job: &RunningJob) -> ApiResult<Result<u64, String>> {
    let combine = job.kind == "combine";
    let room = crate::limits::room(state, job.project, !combine).await?;
    Ok(match room.full {
        Some(reason) if !combine => Err(reason),
        _ if combine => Ok(room.bytes.max(COMBINE_ALLOWANCE)),
        _ => Ok(room.bytes),
    })
}

async fn job_receive_pack(State(state): State<AppState>, Path(id): Path<Uuid>, headers: HeaderMap, body: Body) -> Response {
    let job: RunningJob = match authorize_job(&state, &headers, id).await {
        Ok(j) => j,
        Err(e) => return e.into_response(),
    };
    let Some((rule, protected)) = job.push_rule() else { return job_forbidden() };
    let max_input = match job_push_room(&state, &job).await {
        Ok(Ok(n)) => n,
        Ok(Err(reason)) => return crate::limits::storage_full(reason).into_response(),
        Err(e) => return e.into_response(),
    };
    let mut cmd = git_command(&state, Service::Receive, true, Some(max_input));
    cmd.arg("--stateless-rpc").arg(crate::repos::git_dir(&state.cfg, job.project));
    // The hook checks a job like a lead on a protected option, within the refs the job may touch.
    cmd.env("NICGIT_ROLE", Role::Lead.as_str())
        .env("NICGIT_PROTECTED", protected.join("\u{1f}"))
        .env("NICGIT_MAX_FILE_BYTES", crate::limits::bytes(state.cfg.max_file_mb).to_string())
        .env("NICGIT_GIT", &state.cfg.git)
        .env("NICGIT_JOB_REFS", rule);
    finish_job_rpc(cmd, Service::Receive, &headers, body)
}

/// A job's own flow records what it changed when it finishes, so nothing happens after the process.
fn finish_job_rpc(cmd: tokio::process::Command, service: Service, headers: &HeaderMap, body: Body) -> Response {
    match start_rpc(cmd, service, headers, body) {
        Ok((mut child, response)) => {
            tokio::spawn(async move {
                let _ = child.wait().await;
            });
            response
        }
        Err(r) => r,
    }
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
