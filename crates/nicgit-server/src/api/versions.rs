// SPDX-License-Identifier: AGPL-3.0-or-later
//! Read-only project history: options, versions, milestones, files and the overview.
use crate::{
    access::{self, Project},
    app::AppState,
    auth::{CurrentUser, MaybeUser},
    error::{ApiResult, AppError},
    gitread::{valid_path, Git, Version},
    jobs,
};
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::header,
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use std::process::Stdio;
use tokio_util::io::ReaderStream;
use nicgit_core::roles::Role;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;

pub fn router() -> Router<AppState> {
    let base = "/api/projects/{owner}/{slug}";
    Router::new()
        .route(&format!("{base}/overview"), get(overview))
        .route(&format!("{base}/options"), get(options))
        .route(&format!("{base}/branches"), post(create_branch))
        .route(&format!("{base}/branches/{{name}}"), delete(delete_branch))
        .route(&format!("{base}/versions"), get(versions))
        .route(&format!("{base}/versions/{{rev}}"), get(version))
        .route(&format!("{base}/tags"), get(tags))
        .route(&format!("{base}/archive"), get(archive))
        .route(&format!("{base}/files"), get(files))
        .route(&format!("{base}/blob"), get(blob))
}

pub async fn load(state: &AppState, user: &MaybeUser, owner: &str, slug: &str) -> ApiResult<(Project, Role, Git)> {
    let (p, role) = access::load(state, owner, slug, user.0.as_ref().map(|u| u.id), Role::Viewer).await?;
    let git = Git::new(&p.repo_dir(state));
    Ok((p, role, git))
}

/// Decoded display name of an option: the desktop app percent-encodes option names in refs ("Scheme%20B").
pub fn option_label(name: &str) -> String {
    let b = name.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&name[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| name.to_string())
}

/// Done summary jobs for these commits, by commit.
pub async fn summaries(state: &AppState, project: &Project, commits: &[String]) -> ApiResult<HashMap<String, Value>> {
    let keys: Vec<String> = commits.iter().map(|c| jobs::key(project, "summary", &[c])).collect();
    let rows: Vec<(Value, Value)> = sqlx::query_as("SELECT args, result FROM jobs WHERE cache_key = ANY($1) AND status = 'done'")
        .bind(&keys)
        .fetch_all(&state.db)
        .await?;
    Ok(rows.into_iter().filter_map(|(a, r)| Some((a["rev"].as_str()?.to_string(), r))).collect())
}

async fn option_list(state: &AppState, p: &Project, git: &Git) -> ApiResult<Vec<Value>> {
    let protected = crate::rules::protected_options(&state.db, p.id).await?;
    let mut out = Vec::new();
    for name in git.options().await? {
        let head = git.resolve(&name).await?;
        let latest = git.log(&head, 1).await?.into_iter().next();
        out.push(json!({
            "name": name, "label": option_label(&name), "head": head,
            "time": latest.as_ref().map(|v| v.time), "message": latest.as_ref().map(|v| v.message.clone()),
            "author": latest.as_ref().map(|v| v.author.clone()),
            "is_default": name == p.default_option, "is_agent": name.starts_with("agent/"),
            "protected": protected.contains(&name),
        }));
    }
    out.sort_by(|a, b| b["is_default"].as_bool().cmp(&a["is_default"].as_bool()).then(b["time"].as_i64().cmp(&a["time"].as_i64())));
    Ok(out)
}

async fn options(State(state): State<AppState>, user: MaybeUser, Path((owner, slug)): Path<(String, String)>) -> ApiResult<Json<Value>> {
    let (p, _, git) = load(&state, &user, &owner, &slug).await?;
    Ok(Json(json!(option_list(&state, &p, &git).await?)))
}

#[derive(Deserialize)]
struct NewBranch {
    name: String,
    from: Option<String>,
}

/// Create a design option (a branch) at `from` (the default option when absent).
async fn create_branch(
    State(state): State<AppState>,
    user: CurrentUser,
    Path((owner, slug)): Path<(String, String)>,
    Json(b): Json<NewBranch>,
) -> ApiResult<Json<Value>> {
    let (p, _) = access::load(&state, &owner, &slug, Some(user.id), Role::Member).await?;
    let display = b.name.trim();
    if display.is_empty() || display.len() > 100 {
        return Err(AppError::bad_request("give the option a name"));
    }
    // The desktop app stores spaces percent-encoded in git refs ("Scheme%20B"); keep that convention.
    let name = display.replace(' ', "%20");
    let check = tokio::process::Command::new(&state.cfg.git).args(["check-ref-format", "--branch", &name]).status().await?;
    if !check.success() || name.starts_with("agent/") {
        return Err(AppError::bad_request("that is not a valid option name"));
    }
    let git = Git::new(&p.repo_dir(&state));
    let from = b.from.filter(|f| !f.is_empty()).unwrap_or_else(|| p.default_option.clone());
    let commit = git.resolve(&from).await.map_err(|_| AppError::bad_request("the starting option does not exist"))?;
    crate::git::run(&state.cfg.git, &p.repo_dir(&state).join(".git"), &["branch", &name, &commit])
        .await
        .map_err(|e| AppError::bad_request(e.to_string()))?;
    crate::audit::record(&state.db, Some(p.owner_id), Some(user.id), "option.create", &p.full_name(), json!({"option": name, "from": from})).await?;
    Ok(Json(json!({"name": name, "label": display, "head": commit})))
}

/// Delete a design option. The default option and protected options cannot be deleted.
async fn delete_branch(
    State(state): State<AppState>,
    user: CurrentUser,
    Path((owner, slug, name)): Path<(String, String, String)>,
) -> ApiResult<Json<Value>> {
    let (p, role) = access::load(&state, &owner, &slug, Some(user.id), Role::Member).await?;
    if name == p.default_option {
        return Err(AppError::bad_request("the default option cannot be deleted"));
    }
    let protected = crate::rules::protected_options(&state.db, p.id).await?;
    if protected.contains(&name) && role < Role::Lead {
        return Err(AppError::forbidden("this option is protected; a lead must change it"));
    }
    crate::git::run(&state.cfg.git, &p.repo_dir(&state).join(".git"), &["branch", "-D", &name])
        .await
        .map_err(|e| AppError::bad_request(e.to_string()))?;
    crate::audit::record(&state.db, Some(p.owner_id), Some(user.id), "option.delete", &p.full_name(), json!({"option": name})).await?;
    Ok(Json(json!({"ok": true})))
}

#[derive(Deserialize)]
struct VersionsQuery {
    option: Option<String>,
    before: Option<String>,
    limit: Option<usize>,
}

fn with_summary(v: &Version, s: Option<&Value>) -> Value {
    let mut j = json!(v);
    j["summary"] = s.cloned().unwrap_or(Value::Null);
    j
}

async fn versions(
    State(state): State<AppState>,
    user: MaybeUser,
    Path((owner, slug)): Path<(String, String)>,
    Query(q): Query<VersionsQuery>,
) -> ApiResult<Json<Value>> {
    let (p, _, git) = load(&state, &user, &owner, &slug).await?;
    let option = q.option.unwrap_or_else(|| p.default_option.clone());
    let start = match &q.before {
        Some(b) => git.resolve(b).await?,
        None => match git.resolve(&option).await {
            Ok(h) => h,
            Err(e) if e.status == axum::http::StatusCode::NOT_FOUND => return Ok(Json(json!([]))),
            Err(e) => return Err(e),
        },
    };
    let limit = q.limit.unwrap_or(50).clamp(1, 500);
    let mut list = git.log(&start, limit + usize::from(q.before.is_some())).await?;
    if q.before.is_some() && !list.is_empty() {
        list.remove(0);
    }
    let sums = summaries(&state, &p, &list.iter().map(|v| v.id.clone()).collect::<Vec<_>>()).await?;
    Ok(Json(json!(list.iter().map(|v| with_summary(v, sums.get(&v.id))).collect::<Vec<_>>())))
}

/// The JSON of a project-level file in a version (`settings.json`, `kimbim.json`), if present and valid.
pub async fn json_file(git: &Git, commit: &str, path: &str) -> ApiResult<Option<Value>> {
    Ok(git.blob(commit, path).await?.and_then(|b| serde_json::from_slice(&b).ok()))
}

async fn version(
    State(state): State<AppState>,
    user: MaybeUser,
    Path((owner, slug, rev)): Path<(String, String, String)>,
) -> ApiResult<Json<Value>> {
    let (p, _, git) = load(&state, &user, &owner, &slug).await?;
    let commit = git.resolve(&rev).await?;
    let v = git.log(&commit, 1).await?.into_iter().next().ok_or_else(AppError::not_found)?;
    let sums = summaries(&state, &p, std::slice::from_ref(&commit)).await?;
    let mut out = with_summary(&v, sums.get(&commit));
    out["counts"] = git.counts(&commit).await?;
    let meta = json_file(&git, &commit, "kimbim.json").await?;
    out["schema"] = meta.as_ref().and_then(|m| m.get("schema").cloned()).unwrap_or(Value::Null);
    out["format"] = meta.as_ref().and_then(|m| m.get("k").cloned()).unwrap_or(Value::Null);
    out["settings"] = json_file(&git, &commit, "settings.json").await?.unwrap_or(Value::Null);
    out["checks"] = json_file(&git, &commit, "checks.json").await?.unwrap_or(Value::Null);
    Ok(Json(out))
}

#[derive(Deserialize)]
struct ArchiveQuery {
    rev: Option<String>,
}

/// A source archive of any version, tag or option: `git archive` as a zip (GitHub's "Download ZIP").
async fn archive(
    State(state): State<AppState>,
    user: MaybeUser,
    Path((owner, slug)): Path<(String, String)>,
    Query(q): Query<ArchiveQuery>,
) -> ApiResult<Response> {
    let (p, _, git) = load(&state, &user, &owner, &slug).await?;
    let rev = q.rev.filter(|r| !r.is_empty()).unwrap_or_else(|| p.default_option.clone());
    let commit = git.resolve(&rev).await.map_err(|_| AppError::not_found())?;
    let mut cmd = tokio::process::Command::new(&state.cfg.git);
    cmd.arg("--git-dir")
        .arg(p.repo_dir(&state).join(".git"))
        .args(["archive", "--format=zip", &format!("--prefix={}/", p.slug), &commit])
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = cmd.spawn().map_err(AppError::internal)?;
    let stdout = child.stdout.take().expect("piped");
    tokio::spawn(async move {
        let _ = child.wait().await;
    });
    Ok((
        [
            (header::CONTENT_TYPE, "application/zip".to_string()),
            (header::CONTENT_DISPOSITION, format!("attachment; filename=\"{}-{}.zip\"", p.slug, rev)),
        ],
        Body::from_stream(ReaderStream::new(stdout)),
    )
        .into_response())
}

async fn tags(State(state): State<AppState>, user: MaybeUser, Path((owner, slug)): Path<(String, String)>) -> ApiResult<Json<Value>> {
    let (_, _, git) = load(&state, &user, &owner, &slug).await?;
    if git.options().await?.is_empty() {
        return Ok(Json(json!([])));
    }
    Ok(Json(json!(git.tags().await?)))
}

#[derive(Deserialize)]
struct FilesQuery {
    rev: Option<String>,
    path: Option<String>,
}

async fn files(
    State(state): State<AppState>,
    user: MaybeUser,
    Path((owner, slug)): Path<(String, String)>,
    Query(q): Query<FilesQuery>,
) -> ApiResult<Json<Value>> {
    let (p, _, git) = load(&state, &user, &owner, &slug).await?;
    let rev = q.rev.unwrap_or_else(|| p.default_option.clone());
    let commit = match git.resolve(&rev).await {
        Ok(c) => c,
        Err(e) if e.status == axum::http::StatusCode::NOT_FOUND => return Ok(Json(json!({"entries": [], "empty": true}))),
        Err(e) => return Err(e),
    };
    let path = q.path.unwrap_or_default().trim_matches('/').to_string();
    if !path.is_empty() && !valid_path(&path) {
        return Err(AppError::not_found());
    }
    let spec = if path.is_empty() { commit.clone() } else { format!("{commit}:{path}") };
    let listing = crate::git::run(&state.cfg.git, &p.repo_dir(&state).join(".git"), &["ls-tree", "-l", &spec, "--"])
        .await
        .map_err(|_| AppError::not_found())?;
    let entries: Vec<Value> = listing
        .lines()
        .filter_map(|l| {
            let (meta, name) = l.split_once('\t')?;
            let f: Vec<&str> = meta.split_whitespace().collect();
            Some(json!({"name": name, "type": if f.get(1) == Some(&"tree") { "dir" } else { "file" }, "size": f.get(3).and_then(|s| s.parse::<u64>().ok())}))
        })
        .collect();
    Ok(Json(json!({"commit": commit, "path": path, "entries": entries})))
}

#[derive(Deserialize)]
struct BlobQuery {
    rev: String,
    path: String,
}

async fn blob(
    State(state): State<AppState>,
    user: MaybeUser,
    Path((owner, slug)): Path<(String, String)>,
    Query(q): Query<BlobQuery>,
) -> ApiResult<Response> {
    let (_, _, git) = load(&state, &user, &owner, &slug).await?;
    let commit = git.resolve(&q.rev).await?;
    if !valid_path(&q.path) {
        return Err(AppError::not_found());
    }
    let bytes = git.blob(&commit, &q.path).await?.ok_or_else(AppError::not_found)?;
    let ct = if q.path.ends_with(".json") { "application/json" } else { "application/octet-stream" };
    Ok(([(header::CONTENT_TYPE, ct), (header::X_CONTENT_TYPE_OPTIONS, "nosniff")], bytes).into_response())
}

async fn overview(State(state): State<AppState>, user: MaybeUser, Path((owner, slug)): Path<(String, String)>) -> ApiResult<Json<Value>> {
    let (p, role, git) = load(&state, &user, &owner, &slug).await?;
    let options = option_list(&state, &p, &git).await?;
    let open: (i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM proposals WHERE project_id = $1 AND state = 'open'), \
                (SELECT count(*) FROM issues WHERE project_id = $1 AND state = 'open')",
    )
    .bind(p.id)
    .fetch_one(&state.db)
    .await?;
    let mut out = json!({
        "project": p.to_json(Some(role)), "role": role, "options": options, "empty": options.is_empty(),
        "open_proposals": open.0, "open_issues": open.1,
        "clone_url": format!("{}/{}.git", state.cfg.public_url.trim_end_matches('/'), p.full_name()),
    });
    let head = options.iter().find(|o| o["is_default"] == true).or(options.first()).and_then(|o| o["head"].as_str()).map(str::to_string);
    if let Some(head) = head {
        let latest = git.log(&head, 1).await?.into_iter().next();
        let sums = summaries(&state, &p, std::slice::from_ref(&head)).await?;
        out["latest"] = latest.map(|v| with_summary(&v, sums.get(&head))).unwrap_or(Value::Null);
        out["counts"] = git.counts(&head).await?;
        let meta = json_file(&git, &head, "kimbim.json").await?;
        out["schema"] = meta.and_then(|m| m.get("schema").cloned()).unwrap_or(Value::Null);
        out["settings"] = json_file(&git, &head, "settings.json").await?.unwrap_or(Value::Null);
        out["milestones"] = json!(git.tags().await?);
    }
    Ok(Json(out))
}
