// SPDX-License-Identifier: AGPL-3.0-or-later
//! The home dashboard: the caller's repositories ordered by last activity, plus the recent releases,
//! proposals, issues, checks, posts and activity across those repositories.
use crate::{
    access::ACCESSIBLE_PROJECTS,
    app::AppState,
    auth::CurrentUser,
    error::ApiResult,
};
use axum::{extract::State, routing::get, Json, Router};
use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new().route("/api/me/home", get(home))
}

#[derive(sqlx::FromRow)]
struct RepoRow {
    id: Uuid,
    owner: String,
    owner_kind: String,
    slug: String,
    description: String,
    visibility: String,
    default_option: String,
    created_at: DateTime<Utc>,
    last_activity_at: DateTime<Utc>,
    open_proposals: i64,
    open_issues: i64,
}

#[derive(sqlx::FromRow)]
struct PostRow {
    id: Uuid,
    kind: String,
    data: Value,
    read: bool,
    created_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
struct ReleaseRow {
    project: String,
    tag: String,
    title: String,
    phase: String,
    commit: String,
    created_by: String,
    created_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
struct ProposalRow {
    project: String,
    number: i32,
    title: String,
    source: String,
    target: String,
    author: String,
    created_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
struct IssueRow {
    project: String,
    number: i32,
    title: String,
    priority: String,
    author: String,
    assignee: Option<String>,
    created_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
struct RunRow {
    project: String,
    id: Uuid,
    status: String,
    progress: f32,
    result: Option<Value>,
    created_at: DateTime<Utc>,
    finished_at: Option<DateTime<Utc>>,
}

async fn home(State(state): State<AppState>, user: CurrentUser) -> ApiResult<Json<Value>> {
    let repos: Vec<RepoRow> = sqlx::query_as(&format!(
        "SELECT p.id, a.slug AS owner, a.kind AS owner_kind, p.slug, p.description, p.visibility, p.default_option, p.created_at, \
                COALESCE((SELECT max(x.created_at) FROM activity x WHERE x.project_id = p.id), p.created_at) AS last_activity_at, \
                (SELECT count(*) FROM proposals pr WHERE pr.project_id = p.id AND pr.state = 'open') AS open_proposals, \
                (SELECT count(*) FROM issues i WHERE i.project_id = p.id AND i.state = 'open') AS open_issues \
         FROM projects p JOIN accounts a ON a.id = p.owner_id \
         WHERE {ACCESSIBLE_PROJECTS} ORDER BY last_activity_at DESC, p.slug LIMIT 50"
    ))
    .bind(user.id)
    .fetch_all(&state.db)
    .await?;

    let posts: Vec<PostRow> = sqlx::query_as(
        "SELECT id, kind, data, read, created_at FROM notifications WHERE user_id = $1 ORDER BY created_at DESC LIMIT 8",
    )
    .bind(user.id)
    .fetch_all(&state.db)
    .await?;

    let releases: Vec<ReleaseRow> = sqlx::query_as(&format!(
        "SELECT (a.slug || '/' || p.slug) AS project, m.tag, m.title, m.phase, m.commit_id AS commit, c.slug AS created_by, m.created_at \
         FROM milestones m JOIN projects p ON p.id = m.project_id JOIN accounts a ON a.id = p.owner_id \
         JOIN accounts c ON c.id = m.created_by \
         WHERE {ACCESSIBLE_PROJECTS} ORDER BY m.created_at DESC LIMIT 8"
    ))
    .bind(user.id)
    .fetch_all(&state.db)
    .await?;

    let proposals: Vec<ProposalRow> = sqlx::query_as(&format!(
        "SELECT (a.slug || '/' || p.slug) AS project, pr.number, pr.title, pr.source, pr.target, au.slug AS author, pr.created_at \
         FROM proposals pr JOIN projects p ON p.id = pr.project_id JOIN accounts a ON a.id = p.owner_id \
         JOIN accounts au ON au.id = pr.author_id \
         WHERE {ACCESSIBLE_PROJECTS} AND pr.state = 'open' ORDER BY pr.created_at DESC LIMIT 8"
    ))
    .bind(user.id)
    .fetch_all(&state.db)
    .await?;

    let issues: Vec<IssueRow> = sqlx::query_as(&format!(
        "SELECT (a.slug || '/' || p.slug) AS project, i.number, i.title, i.priority, au.slug AS author, asg.slug AS assignee, i.created_at \
         FROM issues i JOIN projects p ON p.id = i.project_id JOIN accounts a ON a.id = p.owner_id \
         JOIN accounts au ON au.id = i.author_id LEFT JOIN accounts asg ON asg.id = i.assignee_id \
         WHERE {ACCESSIBLE_PROJECTS} AND i.state = 'open' ORDER BY i.created_at DESC LIMIT 8"
    ))
    .bind(user.id)
    .fetch_all(&state.db)
    .await?;

    let runs: Vec<RunRow> = sqlx::query_as(&format!(
        "SELECT (a.slug || '/' || p.slug) AS project, j.id, j.status, j.progress, j.result, j.created_at, j.finished_at \
         FROM jobs j JOIN projects p ON p.id = j.project_id JOIN accounts a ON a.id = p.owner_id \
         WHERE {ACCESSIBLE_PROJECTS} AND j.kind = 'checks' \
         ORDER BY COALESCE(j.finished_at, j.started_at, j.created_at) DESC LIMIT 8"
    ))
    .bind(user.id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(json!({
        "repos": repos.into_iter().map(|r| json!({
            "id": r.id, "owner": r.owner, "owner_kind": r.owner_kind, "slug": r.slug,
            "full_name": format!("{}/{}", r.owner, r.slug), "description": r.description,
            "visibility": r.visibility, "default_option": r.default_option,
            "created_at": r.created_at, "last_activity_at": r.last_activity_at,
            "open_proposals": r.open_proposals, "open_issues": r.open_issues,
        })).collect::<Vec<_>>(),
        "posts": posts.into_iter().map(|p| json!({
            "id": p.id, "kind": p.kind, "data": p.data, "read": p.read, "created_at": p.created_at,
        })).collect::<Vec<_>>(),
        "releases": releases.into_iter().map(|r| json!({
            "project": r.project, "tag": r.tag, "title": r.title, "phase": r.phase,
            "commit": r.commit, "created_by": r.created_by, "created_at": r.created_at,
        })).collect::<Vec<_>>(),
        "packages": [],
        "proposals": proposals.into_iter().map(|p| json!({
            "project": p.project, "number": p.number, "title": p.title, "source": p.source,
            "target": p.target, "author": p.author, "created_at": p.created_at,
        })).collect::<Vec<_>>(),
        "issues": issues.into_iter().map(|i| json!({
            "project": i.project, "number": i.number, "title": i.title, "priority": i.priority,
            "author": i.author, "assignee": i.assignee, "created_at": i.created_at,
        })).collect::<Vec<_>>(),
        "runs": runs.into_iter().map(|r| json!({
            "project": r.project, "id": r.id, "status": r.status, "progress": r.progress,
            "result": r.result, "created_at": r.created_at, "finished_at": r.finished_at,
        })).collect::<Vec<_>>(),
    })))
}
