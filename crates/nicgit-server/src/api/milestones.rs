// SPDX-License-Identifier: AGPL-3.0-or-later
//! Milestones: a version marked as a project phase (an annotated git tag plus a sign-off).
use crate::{
    access,
    app::AppState,
    auth::{CurrentUser, MaybeUser},
    error::{ApiResult, AppError},
    gitread::Git,
};
use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use nicgit_core::roles::Role;
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

pub const PHASES: [&str; 7] = ["concept", "schematic", "design_development", "permit", "tender", "construction", "as_built"];

pub fn router() -> Router<AppState> {
    let base = "/api/projects/{owner}/{slug}/milestones";
    Router::new().route(base, get(list).post(create)).route(&format!("{base}/{{id}}/signoff"), post(signoff))
}

fn valid_tag(t: &str) -> bool {
    (1..=100).contains(&t.len())
        && !t.starts_with(['-', '.'])
        && !t.ends_with(".lock")
        && !t.contains("..")
        && t.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

async fn list(State(state): State<AppState>, MaybeUser(user): MaybeUser, Path((owner, slug)): Path<(String, String)>) -> ApiResult<Json<Value>> {
    let (p, _) = access::load(&state, &owner, &slug, user.map(|u| u.id), Role::Viewer).await?;
    let rows: Vec<(Uuid, String, String, String, String, String, String, Option<String>, Option<DateTime<Utc>>, DateTime<Utc>)> = sqlx::query_as(
        "SELECT m.id, m.phase, m.tag, m.commit_id, m.title, m.notes, c.slug, s.slug, m.signed_off_at, m.created_at \
         FROM milestones m JOIN accounts c ON c.id = m.created_by LEFT JOIN accounts s ON s.id = m.signed_off_by \
         WHERE m.project_id = $1 ORDER BY m.created_at DESC",
    )
    .bind(p.id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(json!(rows
        .into_iter()
        .map(|(id, phase, tag, commit, title, notes, by, signed, signed_at, at)| json!({
            "id": id, "phase": phase, "tag": tag, "commit": commit, "title": title, "notes": notes,
            "created_by": by, "signed_off_by": signed, "signed_off_at": signed_at, "created_at": at,
        }))
        .collect::<Vec<_>>())))
}

#[derive(Deserialize)]
struct NewMilestone {
    phase: String,
    tag: String,
    rev: String,
    title: String,
    #[serde(default)]
    notes: String,
}

async fn create(State(state): State<AppState>, user: CurrentUser, Path((owner, slug)): Path<(String, String)>, Json(b): Json<NewMilestone>) -> ApiResult<Json<Value>> {
    let (p, _) = access::load(&state, &owner, &slug, Some(user.id), Role::Lead).await?;
    if !PHASES.contains(&b.phase.as_str()) {
        return Err(AppError::bad_request("unknown phase"));
    }
    if !valid_tag(&b.tag) {
        return Err(AppError::bad_request("milestone names use letters, digits, '-', '_' and '.'"));
    }
    let title = b.title.trim();
    if title.is_empty() {
        return Err(AppError::bad_request("give the milestone a title"));
    }
    let git = Git::new(&p.repo_dir(&state));
    let commit = git.resolve(&b.rev).await?;
    let exists: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM milestones WHERE project_id = $1 AND tag = $2)")
        .bind(p.id)
        .bind(&b.tag)
        .fetch_one(&state.db)
        .await?;
    if exists || git.tags().await?.iter().any(|t| t["name"] == b.tag.as_str()) {
        return Err(AppError::conflict("taken", "a milestone with that name already exists"));
    }
    let (name, email): (String, String) = sqlx::query_as("SELECT a.display_name, u.email FROM users u JOIN accounts a ON a.id = u.account_id WHERE u.account_id = $1")
        .bind(user.id)
        .fetch_one(&state.db)
        .await?;
    let message = if b.notes.trim().is_empty() { title.to_string() } else { format!("{title}\n\n{}", b.notes.trim()) };
    let out = tokio::process::Command::new(&state.cfg.git)
        .arg("--git-dir")
        .arg(p.repo_dir(&state).join(".git"))
        .args(["-c", &format!("user.name={name}"), "-c", &format!("user.email={email}"), "tag", "-a", &b.tag, &commit, "-m", &message])
        .output()
        .await?;
    if !out.status.success() {
        return Err(AppError::internal(String::from_utf8_lossy(&out.stderr)));
    }
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO milestones (id, project_id, phase, tag, commit_id, title, notes, created_by) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)")
        .bind(id)
        .bind(p.id)
        .bind(&b.phase)
        .bind(&b.tag)
        .bind(&commit)
        .bind(title)
        .bind(b.notes.trim())
        .bind(user.id)
        .execute(&state.db)
        .await?;
    crate::activity::record(&state.db, p.id, Some(user.id), "milestone.created", json!({"tag": b.tag, "phase": b.phase})).await?;
    crate::audit::record(&state.db, Some(p.owner_id), Some(user.id), "milestone.create", &format!("{}:{}", p.full_name(), b.tag), json!({"commit": commit})).await?;
    Ok(Json(json!({"id": id, "tag": b.tag, "commit": commit})))
}

async fn signoff(State(state): State<AppState>, user: CurrentUser, Path((owner, slug, id)): Path<(String, String, Uuid)>) -> ApiResult<Json<Value>> {
    let (p, _) = access::load(&state, &owner, &slug, Some(user.id), Role::Lead).await?;
    let n = sqlx::query("UPDATE milestones SET signed_off_by = $3, signed_off_at = now() WHERE id = $1 AND project_id = $2")
        .bind(id)
        .bind(p.id)
        .bind(user.id)
        .execute(&state.db)
        .await?;
    if n.rows_affected() == 0 {
        return Err(AppError::not_found());
    }
    crate::audit::record(&state.db, Some(p.owner_id), Some(user.id), "milestone.signoff", &p.full_name(), json!({"id": id})).await?;
    Ok(Json(json!({"ok": true})))
}
