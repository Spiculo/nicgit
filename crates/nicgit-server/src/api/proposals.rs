// SPDX-License-Identifier: AGPL-3.0-or-later
//! Proposals: combine one design option into another, after review (spec section 9).
use crate::{
    access::{self, Project},
    api::jobs::respond,
    app::AppState,
    auth::{CurrentUser, MaybeUser},
    error::{ApiResult, AppError},
    gitread::Git,
    jobs,
    proposals::{self, Proposal},
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Response,
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use nicgit_core::{
    conflicts::{self, ConflictMode, Side},
    roles::Role,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::Duration;
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    let base = "/api/projects/{owner}/{slug}/proposals";
    Router::new()
        .route(base, get(list).post(create))
        .route(&format!("{base}/{{number}}"), get(detail))
        .route(&format!("{base}/{{number}}/reviews"), post(review))
        .route(&format!("{base}/{{number}}/comments"), post(comment))
        .route(&format!("{base}/{{number}}/close"), post(close))
        .route(&format!("{base}/{{number}}/preview"), get(preview))
        .route(&format!("{base}/{{number}}/combine"), post(combine))
        .route(&format!("{base}/{{number}}/ai-summary"), post(ai_summary))
}

async fn load(state: &AppState, owner: &str, slug: &str, user: Option<Uuid>, need: Role) -> ApiResult<(Project, Role, Git)> {
    let (p, role) = access::load(state, owner, slug, user, need).await?;
    let git = Git::new(&p.repo_dir(state));
    Ok((p, role, git))
}

async fn load_proposal(state: &AppState, project: &Project, number: i32) -> ApiResult<Proposal> {
    proposals::get(state, project.id, number).await?.ok_or_else(AppError::not_found)
}

#[derive(Deserialize)]
struct ListQuery {
    state: Option<String>,
}

async fn list(
    State(state): State<AppState>,
    MaybeUser(user): MaybeUser,
    Path((owner, slug)): Path<(String, String)>,
    Query(q): Query<ListQuery>,
) -> ApiResult<Json<Value>> {
    let (p, _, _) = load(&state, &owner, &slug, user.map(|u| u.id), Role::Viewer).await?;
    let rows: Vec<Proposal> = sqlx::query_as(&format!(
        "SELECT {} FROM proposals p JOIN accounts a ON a.id = p.author_id \
         WHERE p.project_id = $1 AND ($2::text IS NULL OR p.state = $2) ORDER BY p.number DESC",
        proposals::COLUMNS
    ))
    .bind(p.id)
    .bind(q.state)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(json!(rows)))
}

#[derive(Deserialize)]
struct NewProposal {
    title: String,
    #[serde(default)]
    body: String,
    source: String,
    target: String,
}

async fn create(
    State(state): State<AppState>,
    user: CurrentUser,
    Path((owner, slug)): Path<(String, String)>,
    Json(b): Json<NewProposal>,
) -> ApiResult<Json<Value>> {
    let (p, _, git) = load(&state, &owner, &slug, Some(user.id), Role::Member).await?;
    let title = b.title.trim();
    if title.is_empty() || title.len() > 200 {
        return Err(AppError::bad_request("give the proposal a title"));
    }
    if b.source == b.target {
        return Err(AppError::bad_request("an option cannot be combined into itself"));
    }
    let options = git.options().await?;
    if !options.contains(&b.source) || !options.contains(&b.target) {
        return Err(AppError::not_found());
    }
    let (source_head, target_head) = (git.resolve(&b.source).await?, git.resolve(&b.target).await?);
    let id = Uuid::new_v4();
    let mut tx = state.db.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1::text))").bind(p.id.to_string()).execute(&mut *tx).await?;
    let number: i32 = sqlx::query_scalar("SELECT coalesce(max(number), 0) + 1 FROM proposals WHERE project_id = $1").bind(p.id).fetch_one(&mut *tx).await?;
    sqlx::query(
        "INSERT INTO proposals (id, project_id, number, title, body, source, target, author_id, source_head, target_head) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
    )
    .bind(id)
    .bind(p.id)
    .bind(number)
    .bind(title)
    .bind(b.body.trim())
    .bind(&b.source)
    .bind(&b.target)
    .bind(user.id)
    .bind(&source_head)
    .bind(&target_head)
    .execute(&mut *tx)
    .await?;
    crate::activity::record(&mut *tx, p.id, Some(user.id), "proposal.opened", json!({"number": number, "title": title})).await?;
    tx.commit().await?;
    proposals::set_ref(&state, &p, number, &source_head).await?;
    // Ask the people who approve into this option.
    let reviewers: Vec<Uuid> = sqlx::query_scalar(
        "SELECT user_id FROM project_members WHERE project_id = $1 AND role IN ('reviewer','lead','admin') AND user_id <> $2",
    )
    .bind(p.id)
    .bind(user.id)
    .fetch_all(&state.db)
    .await?;
    for r in reviewers {
        crate::notify::send(&state.db, r, "proposal.review_requested", json!({"project": p.full_name(), "number": number, "title": title})).await?;
    }
    crate::jobs::enqueue(&state.db, p.id, "merge_preview", json!({"target": b.target, "source": source_head}), Some(&preview_key(&p, &target_head, &source_head)))
        .await?;
    crate::proposals::queue_required_checks(&state, &p, &b.target, &source_head).await?;
    let created = load_proposal(&state, &p, number).await?;
    Ok(Json(json!(created)))
}

fn preview_key(p: &Project, target_head: &str, source_head: &str) -> String {
    jobs::key(p, "merge_preview", &[target_head, source_head])
}

async fn detail(
    State(state): State<AppState>,
    MaybeUser(user): MaybeUser,
    Path((owner, slug, number)): Path<(String, String, i32)>,
) -> ApiResult<Json<Value>> {
    let uid = user.as_ref().map(|u| u.id);
    let (p, role, git) = load(&state, &owner, &slug, uid, Role::Viewer).await?;
    let prop = load_proposal(&state, &p, number).await?;
    let (eval, rule, protected, settings_changed) = proposals::evaluate(&state, &p, &prop, &git).await?;
    let reviews = proposals::reviews(&state, prop.id).await?;
    let comments: Vec<(Uuid, String, String, Option<Value>, Option<String>, DateTime<Utc>)> = sqlx::query_as(
        "SELECT c.id, a.slug, c.body, c.pin, c.version, c.created_at FROM comments c JOIN accounts a ON a.id = c.author_id \
         WHERE c.proposal_id = $1 ORDER BY c.created_at",
    )
    .bind(prop.id)
    .fetch_all(&state.db)
    .await?;
    let comments: Vec<Value> = comments
        .into_iter()
        .map(|(id, author, body, pin, version, at)| json!({"id": id, "author": author, "body": body, "pin": pin, "version": version, "created_at": at}))
        .collect();
    let can_combine = prop.state == "open" && role >= if protected { Role::Lead } else { Role::Member };
    let ai = crate::ai::proposal_summary_cached(&state, &p, &prop).await?;
    let checks = crate::actions::status(&state, &p, &prop.source_head, &rule.required_checks).await?;
    Ok(Json(json!({
        "proposal": prop, "reviews": reviews, "comments": comments, "rule": rule, "protected": protected,
        "approvals": eval, "settings_changed": settings_changed, "can_combine": can_combine, "role": role,
        "can_review": uid.is_some() && role >= Role::Reviewer, "ai_summary": ai, "checks": checks,
    })))
}

#[derive(Deserialize)]
struct ReviewBody {
    verdict: String,
    #[serde(default)]
    body: String,
}

async fn review(
    State(state): State<AppState>,
    user: CurrentUser,
    Path((owner, slug, number)): Path<(String, String, i32)>,
    Json(b): Json<ReviewBody>,
) -> ApiResult<Json<Value>> {
    let (p, _, _) = load(&state, &owner, &slug, Some(user.id), Role::Reviewer).await?;
    let prop = load_proposal(&state, &p, number).await?;
    if prop.state != "open" {
        return Err(AppError::conflict("not_open", "this proposal is no longer open"));
    }
    if nicgit_core::approvals::Verdict::parse(&b.verdict).is_none() {
        return Err(AppError::bad_request("verdict is approve, changes or comment"));
    }
    if b.verdict != "comment" && prop.author_id == user.id {
        return Err(AppError::bad_request("authors cannot approve or reject their own proposal"));
    }
    sqlx::query("INSERT INTO reviews (id, proposal_id, user_id, verdict, body, source_head) VALUES ($1, $2, $3, $4, $5, $6)")
        .bind(Uuid::new_v4())
        .bind(prop.id)
        .bind(user.id)
        .bind(&b.verdict)
        .bind(b.body.trim())
        .bind(&prop.source_head)
        .execute(&state.db)
        .await?;
    crate::activity::record(&state.db, p.id, Some(user.id), "proposal.reviewed", json!({"number": number, "verdict": b.verdict})).await?;
    crate::notify::send(&state.db, prop.author_id, "proposal.reviewed", json!({"project": p.full_name(), "number": number, "title": prop.title, "verdict": b.verdict, "by": user.slug})).await?;
    Ok(Json(json!({"ok": true})))
}

#[derive(Deserialize)]
struct CommentBody {
    body: String,
    pin: Option<Value>,
    version: Option<String>,
}

async fn comment(
    State(state): State<AppState>,
    user: CurrentUser,
    Path((owner, slug, number)): Path<(String, String, i32)>,
    Json(b): Json<CommentBody>,
) -> ApiResult<Json<Value>> {
    let (p, _, _) = load(&state, &owner, &slug, Some(user.id), Role::Viewer).await?;
    let prop = load_proposal(&state, &p, number).await?;
    let body = b.body.trim();
    if body.is_empty() || body.len() > 20_000 {
        return Err(AppError::bad_request("write a comment"));
    }
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO comments (id, project_id, proposal_id, author_id, body, pin, version) VALUES ($1, $2, $3, $4, $5, $6, $7)")
        .bind(id)
        .bind(p.id)
        .bind(prop.id)
        .bind(user.id)
        .bind(body)
        .bind(&b.pin)
        .bind(&b.version)
        .execute(&state.db)
        .await?;
    Ok(Json(json!({"id": id})))
}

async fn close(State(state): State<AppState>, user: CurrentUser, Path((owner, slug, number)): Path<(String, String, i32)>) -> ApiResult<Json<Value>> {
    let (p, role, _) = load(&state, &owner, &slug, Some(user.id), Role::Viewer).await?;
    let prop = load_proposal(&state, &p, number).await?;
    if prop.author_id != user.id && role < Role::Lead {
        return Err(AppError::forbidden("only the author or a lead can close a proposal"));
    }
    sqlx::query("UPDATE proposals SET state = 'closed', updated_at = now() WHERE id = $1 AND state = 'open'").bind(prop.id).execute(&state.db).await?;
    crate::activity::record(&state.db, p.id, Some(user.id), "proposal.closed", json!({"number": number})).await?;
    Ok(Json(json!({"ok": true})))
}

async fn preview(
    State(state): State<AppState>,
    MaybeUser(user): MaybeUser,
    Path((owner, slug, number)): Path<(String, String, i32)>,
) -> ApiResult<Response> {
    let (p, _, git) = load(&state, &owner, &slug, user.map(|u| u.id), Role::Viewer).await?;
    let prop = load_proposal(&state, &p, number).await?;
    let (target_head, source_head) = current_heads(&git, &prop).await?;
    let job = jobs::enqueue(&state.db, p.id, "merge_preview", json!({"target": prop.target, "source": source_head}), Some(&preview_key(&p, &target_head, &source_head)))
        .await?;
    Ok(respond(job))
}

async fn current_heads(git: &Git, prop: &Proposal) -> ApiResult<(String, String)> {
    let target = git.resolve(&prop.target).await?;
    let source = git.resolve(&prop.source).await.unwrap_or_else(|_| prop.source_head.clone());
    Ok((target, source))
}

#[derive(Deserialize)]
struct CombineBody {
    mode: String,
    #[serde(default)]
    picks: HashMap<String, Side>,
    #[serde(default)]
    settings_picks: HashMap<String, Side>,
}

/// Held while a project's options are being combined: one Combine per project at a time.
struct ProjectLock {
    conn: Option<sqlx::pool::PoolConnection<sqlx::Postgres>>,
    key: String,
}

impl ProjectLock {
    async fn try_take(state: &AppState, project: Uuid) -> ApiResult<Option<ProjectLock>> {
        let mut conn = state.db.acquire().await?;
        let key = format!("combine:{project}");
        let ok: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock(hashtext($1))").bind(&key).fetch_one(&mut *conn).await?;
        Ok(ok.then_some(ProjectLock { conn: Some(conn), key }))
    }
    async fn release(mut self) {
        if let Some(mut c) = self.conn.take() {
            let _ = sqlx::query("SELECT pg_advisory_unlock(hashtext($1))").bind(&self.key).execute(&mut *c).await;
        }
    }
}

impl Drop for ProjectLock {
    fn drop(&mut self) {
        // A dropped connection ends its session, which releases the lock too.
        if let Some(c) = self.conn.take() {
            drop(c.detach());
        }
    }
}

const PREVIEW_WAIT: Duration = Duration::from_secs(600);
const COMBINE_WAIT: Duration = Duration::from_secs(900);

async fn combine(
    State(state): State<AppState>,
    user: CurrentUser,
    Path((owner, slug, number)): Path<(String, String, i32)>,
    Json(b): Json<CombineBody>,
) -> ApiResult<Json<Value>> {
    let (p, role, git) = load(&state, &owner, &slug, Some(user.id), Role::Member).await?;
    let prop = load_proposal(&state, &p, number).await?;
    if prop.state != "open" {
        return Err(AppError::conflict("not_open", "this proposal is no longer open"));
    }
    let mode: ConflictMode = b.mode.parse().map_err(AppError::bad_request)?;
    let (eval, rule, protected, _) = proposals::evaluate(&state, &p, &prop, &git).await?;
    if protected && role < Role::Lead {
        return Err(AppError::forbidden(format!("only a lead can combine into the protected option '{}'", prop.target)));
    }
    if !rule.allows(mode) {
        return Err(AppError::conflict("mode_not_allowed", format!("'{}' does not allow settling conflicts this way", prop.target)));
    }
    if !eval.ok {
        return Err(AppError::new(StatusCode::CONFLICT, "approvals_missing", serde_json::to_string(&eval.missing).unwrap_or_default()));
    }
    let checks = crate::actions::status(&state, &p, &prop.source_head, &rule.required_checks).await?;
    match checks["state"].as_str() {
        Some("pending") => return Err(AppError::conflict("checks_pending", "the required checks have not finished on the latest version")),
        Some("failed") => return Err(AppError::conflict("checks_failed", "the required checks failed on the latest version")),
        _ => {}
    }
    let lock = ProjectLock::try_take(&state, p.id).await?.ok_or_else(|| AppError::conflict("busy", "another combine is running on this project"))?;
    let result = run_combine(&state, &p, &prop, &user, &git, mode, &b).await;
    lock.release().await;
    result
}

async fn run_combine(
    state: &AppState,
    p: &Project,
    prop: &Proposal,
    user: &CurrentUser,
    git: &Git,
    mode: ConflictMode,
    b: &CombineBody,
) -> ApiResult<Json<Value>> {
    let (target_head, source_head) = current_heads(git, prop).await?;
    let job = jobs::enqueue(&state.db, p.id, "merge_preview", json!({"target": prop.target, "source": source_head}), Some(&preview_key(p, &target_head, &source_head)))
        .await?;
    let job = jobs::wait(&state.db, job.id, PREVIEW_WAIT).await?;
    if job.failed() {
        return Err(AppError::new(StatusCode::UNPROCESSABLE_ENTITY, "preview_failed", job.error.unwrap_or_default()));
    }
    let preview = job.result.ok_or_else(|| AppError::new(StatusCode::SERVICE_UNAVAILABLE, "engine_busy", "the engine did not answer in time; try again"))?;
    let ids: Vec<String> = preview["conflicts"].as_array().into_iter().flatten().filter_map(|c| c["id"].as_str().map(str::to_string)).collect();
    let resolutions = conflicts::resolve(mode, &ids, &b.picks)
        .map_err(|m| AppError::new(StatusCode::CONFLICT, "conflicts_unresolved", serde_json::to_string(&m.0).unwrap_or_default()))?;
    let settings_keys: Vec<String> =
        preview["settings_conflicts"].as_array().into_iter().flatten().filter_map(|c| c["key"].as_str().or(c.as_str()).map(str::to_string)).collect();
    let settings_resolutions = conflicts::resolve(mode, &settings_keys, &b.settings_picks)
        .map_err(|m| AppError::new(StatusCode::CONFLICT, "conflicts_unresolved", serde_json::to_string(&m.0).unwrap_or_default()))?;

    let (name, email) = crate::auth::commit_identity(&state.db, user.id).await?;
    let approvers: Vec<String> = proposals::reviews(state, prop.id)
        .await?
        .into_iter()
        .filter(|r| r.verdict == "approve" && !r.dismissed)
        .map(|r| r.user)
        .collect();
    let label = crate::api::versions::option_label;
    let mut message = format!(
        "Combine proposal #{}: {}

{} into {}
Proposal: #{}
Mode: {}",
        prop.number,
        prop.title,
        label(&prop.source),
        label(&prop.target),
        prop.number,
        mode.as_str()
    );
    for a in approvers {
        message.push_str(&format!("\nApproved-by: {a}"));
    }
    let job = jobs::enqueue(
        &state.db,
        p.id,
        "combine",
        json!({"target": prop.target, "source": source_head, "resolutions": resolutions, "settings_resolutions": settings_resolutions,
               "message": message, "author": [name, email]}),
        None,
    )
    .await?;
    let job = jobs::wait(&state.db, job.id, COMBINE_WAIT).await?;
    if job.failed() {
        return Err(AppError::new(StatusCode::UNPROCESSABLE_ENTITY, "combine_failed", job.error.unwrap_or_default()));
    }
    let commit = job
        .result
        .as_ref()
        .and_then(|r| r["commit"].as_str())
        .map(str::to_string)
        .ok_or_else(|| AppError::new(StatusCode::SERVICE_UNAVAILABLE, "engine_busy", "the engine did not answer in time; the combine may still finish"))?;
    sqlx::query("UPDATE proposals SET state = 'combined', merge_commit = $2, target_head = $2, source_head = $3, updated_at = now() WHERE id = $1")
        .bind(prop.id)
        .bind(&commit)
        .bind(&source_head)
        .execute(&state.db)
        .await?;
    crate::activity::record(&state.db, p.id, Some(user.id), "proposal.combined", json!({"number": prop.number, "commit": commit, "mode": mode.as_str()})).await?;
    crate::audit::record(&state.db, Some(p.owner_id), Some(user.id), "proposal.combine", &format!("{}#{}", p.full_name(), prop.number), json!({"commit": commit, "mode": mode.as_str()}))
        .await?;
    crate::notify::send(&state.db, prop.author_id, "proposal.combined", json!({"project": p.full_name(), "number": prop.number, "title": prop.title})).await?;
    jobs::enqueue_summary(state, p, &commit).await?;
    proposals::on_option_moved(state, p, &prop.target, &commit).await?;
    Ok(Json(json!({"commit": commit, "status": job.result.as_ref().and_then(|r| r["status"].as_str())})))
}

/// An AI summary of the proposal's changes, from the engine's compare of its two heads (spec section 13).
async fn ai_summary(
    State(state): State<AppState>,
    user: CurrentUser,
    Path((owner, slug, number)): Path<(String, String, i32)>,
) -> ApiResult<Response> {
    let (p, _, _) = load(&state, &owner, &slug, Some(user.id), Role::Reviewer).await?;
    let prop = load_proposal(&state, &p, number).await?;
    if let Some(cached) = crate::ai::proposal_summary_cached(&state, &p, &prop).await? {
        return Ok(axum::response::IntoResponse::into_response(Json(cached)));
    }
    crate::ai::ensure_available(&state, &p).await?;
    let job = jobs::enqueue(
        &state.db,
        p.id,
        "compare",
        json!({"from": prop.target_head, "to": prop.source_head}),
        Some(&jobs::key(&p, "compare", &[&prop.target_head, &prop.source_head])),
    )
    .await?;
    let job = jobs::wait(&state.db, job.id, Duration::from_secs(120)).await?;
    if !job.done() {
        return Ok(respond(job));
    }
    let compare = job.result.unwrap_or_default();
    let lang: String = sqlx::query_scalar("SELECT lang FROM users WHERE account_id = $1").bind(user.id).fetch_one(&state.db).await?;
    let out = crate::ai::summarize_proposal(&state, &p, &prop, &compare, &lang).await?;
    Ok(axum::response::IntoResponse::into_response(Json(out)))
}
