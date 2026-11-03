// SPDX-License-Identifier: AGPL-3.0-or-later
//! Issues: BCF topics pinned to elements and a viewpoint (spec section 9 screens, BCF import/export).
use crate::{
    access::{self, Project},
    app::AppState,
    auth::{CurrentUser, MaybeUser},
    bcf,
    error::{ApiResult, AppError},
};
use axum::{
    extract::{Multipart, Path, Query, State},
    http::header,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use nicgit_core::roles::Role;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    let base = "/api/projects/{owner}/{slug}/issues";
    Router::new()
        .route(base, get(list).post(create))
        .route(&format!("{base}/export.bcfzip"), get(export))
        .route(&format!("{base}/import"), post(import).layer(axum::extract::DefaultBodyLimit::max(MAX_BCF + (1 << 20))))
        .route(&format!("{base}/{{number}}"), get(detail).patch(update))
        .route(&format!("{base}/{{number}}/comments"), post(comment))
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct Issue {
    id: Uuid,
    number: i32,
    title: String,
    body: String,
    state: String,
    priority: String,
    author: String,
    assignee: Option<String>,
    labels: Vec<String>,
    viewpoint: Option<Value>,
    elements: Vec<String>,
    version: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

const COLUMNS: &str = "i.id, i.number, i.title, i.body, i.state, i.priority, au.slug AS author, asg.slug AS assignee, i.labels, \
     i.viewpoint, i.elements, i.version, i.created_at, i.updated_at";
const FROM: &str = "FROM issues i JOIN accounts au ON au.id = i.author_id LEFT JOIN accounts asg ON asg.id = i.assignee_id";
const PRIORITIES: [&str; 4] = ["low", "normal", "high", "critical"];

async fn load(state: &AppState, owner: &str, slug: &str, user: Option<Uuid>, need: Role) -> ApiResult<(Project, Role)> {
    access::load(state, owner, slug, user, need).await
}

async fn issue(state: &AppState, project: Uuid, number: i32) -> ApiResult<Issue> {
    sqlx::query_as(&format!("SELECT {COLUMNS} {FROM} WHERE i.project_id = $1 AND i.number = $2"))
        .bind(project)
        .bind(number)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(AppError::not_found)
}

#[derive(Deserialize)]
struct ListQuery {
    state: Option<String>,
}

async fn list(State(state): State<AppState>, MaybeUser(user): MaybeUser, Path((owner, slug)): Path<(String, String)>, Query(q): Query<ListQuery>) -> ApiResult<Json<Value>> {
    let (p, _) = load(&state, &owner, &slug, user.map(|u| u.id), Role::Viewer).await?;
    let rows: Vec<Issue> = sqlx::query_as(&format!(
        "SELECT {COLUMNS} {FROM} WHERE i.project_id = $1 AND ($2::text IS NULL OR i.state = $2) ORDER BY i.number DESC"
    ))
    .bind(p.id)
    .bind(q.state)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(json!(rows)))
}

/// A user who can read the project, by name (assignees must be able to see what they are given).
async fn member(state: &AppState, project: &Project, slug: &str) -> ApiResult<Uuid> {
    let id = crate::api::orgs::user_id(state, slug).await?;
    if access::project_role(state, Some(id), project).await?.is_none_or(|r| r < Role::Reviewer) {
        return Err(AppError::bad_request(format!("{slug} cannot work on this project")));
    }
    Ok(id)
}

#[derive(Deserialize)]
struct NewIssue {
    title: String,
    #[serde(default)]
    body: String,
    priority: Option<String>,
    assignee: Option<String>,
    #[serde(default)]
    elements: Vec<String>,
    viewpoint: Option<Value>,
    version: Option<String>,
    #[serde(default)]
    labels: Vec<String>,
}

struct Insert<'a> {
    title: &'a str,
    body: &'a str,
    state: &'a str,
    priority: &'a str,
    author: Uuid,
    assignee: Option<Uuid>,
    labels: &'a [String],
    viewpoint: Option<&'a Value>,
    elements: &'a [String],
    version: Option<&'a str>,
}

async fn insert(state: &AppState, project: &Project, i: Insert<'_>) -> ApiResult<(Uuid, i32)> {
    let id = Uuid::new_v4();
    let mut tx = state.db.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1::text))").bind(format!("issues:{}", project.id)).execute(&mut *tx).await?;
    let number: i32 = sqlx::query_scalar("SELECT coalesce(max(number), 0) + 1 FROM issues WHERE project_id = $1").bind(project.id).fetch_one(&mut *tx).await?;
    sqlx::query(
        "INSERT INTO issues (id, project_id, number, title, body, state, priority, author_id, assignee_id, labels, viewpoint, elements, version) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)",
    )
    .bind(id)
    .bind(project.id)
    .bind(number)
    .bind(i.title)
    .bind(i.body)
    .bind(i.state)
    .bind(i.priority)
    .bind(i.author)
    .bind(i.assignee)
    .bind(i.labels)
    .bind(i.viewpoint)
    .bind(i.elements)
    .bind(i.version)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok((id, number))
}

async fn create(State(state): State<AppState>, user: CurrentUser, Path((owner, slug)): Path<(String, String)>, Json(b): Json<NewIssue>) -> ApiResult<Json<Value>> {
    let (p, _) = load(&state, &owner, &slug, Some(user.id), Role::Reviewer).await?;
    let title = b.title.trim();
    if title.is_empty() || title.len() > 300 {
        return Err(AppError::bad_request("give the issue a title"));
    }
    let priority = b.priority.unwrap_or_else(|| "normal".into());
    if !PRIORITIES.contains(&priority.as_str()) {
        return Err(AppError::bad_request("priority is low, normal, high or critical"));
    }
    let assignee = match &b.assignee {
        Some(a) if !a.is_empty() => Some(member(&state, &p, a).await?),
        _ => None,
    };
    let (_, number) = insert(
        &state,
        &p,
        Insert {
            title,
            body: b.body.trim(),
            state: "open",
            priority: &priority,
            author: user.id,
            assignee,
            labels: &b.labels,
            viewpoint: b.viewpoint.as_ref(),
            elements: &b.elements,
            version: b.version.as_deref(),
        },
    )
    .await?;
    crate::activity::record(&state.db, p.id, Some(user.id), "issue.opened", json!({"number": number, "title": title})).await?;
    if let Some(a) = assignee.filter(|a| *a != user.id) {
        crate::notify::send(&state.db, a, "issue.assigned", json!({"project": p.full_name(), "number": number, "title": title})).await?;
    }
    Ok(Json(json!(issue(&state, p.id, number).await?)))
}

async fn detail(State(state): State<AppState>, MaybeUser(user): MaybeUser, Path((owner, slug, number)): Path<(String, String, i32)>) -> ApiResult<Json<Value>> {
    let (p, role) = load(&state, &owner, &slug, user.map(|u| u.id), Role::Viewer).await?;
    let i = issue(&state, p.id, number).await?;
    let comments: Vec<(Uuid, String, String, DateTime<Utc>)> = sqlx::query_as(
        "SELECT c.id, a.slug, c.body, c.created_at FROM comments c JOIN accounts a ON a.id = c.author_id WHERE c.issue_id = $1 ORDER BY c.created_at",
    )
    .bind(i.id)
    .fetch_all(&state.db)
    .await?;
    let comments: Vec<Value> = comments.into_iter().map(|(id, a, b, at)| json!({"id": id, "author": a, "body": b, "created_at": at})).collect();
    Ok(Json(json!({"issue": i, "comments": comments, "role": role})))
}

#[derive(Deserialize)]
struct IssueUpdate {
    title: Option<String>,
    body: Option<String>,
    state: Option<String>,
    priority: Option<String>,
    assignee: Option<String>,
}

async fn update(
    State(state): State<AppState>,
    user: CurrentUser,
    Path((owner, slug, number)): Path<(String, String, i32)>,
    Json(b): Json<IssueUpdate>,
) -> ApiResult<Json<Value>> {
    let (p, role) = load(&state, &owner, &slug, Some(user.id), Role::Reviewer).await?;
    let i = issue(&state, p.id, number).await?;
    if i.author != user.slug && role < Role::Member && i.assignee.as_deref() != Some(&user.slug) {
        return Err(AppError::forbidden("only the author, the assignee or a member can change this issue"));
    }
    if let Some(s) = &b.state {
        if s != "open" && s != "closed" {
            return Err(AppError::bad_request("state is open or closed"));
        }
    }
    if let Some(pr) = &b.priority {
        if !PRIORITIES.contains(&pr.as_str()) {
            return Err(AppError::bad_request("priority is low, normal, high or critical"));
        }
    }
    let assignee = match &b.assignee {
        Some(a) if a.is_empty() => Some(None),
        Some(a) => Some(Some(member(&state, &p, a).await?)),
        None => None,
    };
    sqlx::query(
        "UPDATE issues SET title = coalesce($2, title), body = coalesce($3, body), state = coalesce($4, state), \
         priority = coalesce($5, priority), assignee_id = CASE WHEN $6 THEN $7 ELSE assignee_id END, updated_at = now() WHERE id = $1",
    )
    .bind(i.id)
    .bind(b.title.as_deref().map(str::trim))
    .bind(b.body.as_deref())
    .bind(b.state.as_deref())
    .bind(b.priority.as_deref())
    .bind(assignee.is_some())
    .bind(assignee.flatten())
    .execute(&state.db)
    .await?;
    if let Some(Some(a)) = assignee.filter(|a| *a != Some(user.id)) {
        crate::notify::send(&state.db, a, "issue.assigned", json!({"project": p.full_name(), "number": number, "title": i.title})).await?;
    }
    Ok(Json(json!(issue(&state, p.id, number).await?)))
}

#[derive(Deserialize)]
struct CommentBody {
    body: String,
}

async fn comment(
    State(state): State<AppState>,
    user: CurrentUser,
    Path((owner, slug, number)): Path<(String, String, i32)>,
    Json(b): Json<CommentBody>,
) -> ApiResult<Json<Value>> {
    let (p, _) = load(&state, &owner, &slug, Some(user.id), Role::Viewer).await?;
    let i = issue(&state, p.id, number).await?;
    let body = b.body.trim();
    if body.is_empty() || body.len() > 20_000 {
        return Err(AppError::bad_request("write a comment"));
    }
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO comments (id, project_id, issue_id, author_id, body) VALUES ($1, $2, $3, $4, $5)")
        .bind(id)
        .bind(p.id)
        .bind(i.id)
        .bind(user.id)
        .bind(body)
        .execute(&state.db)
        .await?;
    Ok(Json(json!({"id": id})))
}

async fn export(State(state): State<AppState>, MaybeUser(user): MaybeUser, Path((owner, slug)): Path<(String, String)>) -> ApiResult<Response> {
    let (p, _) = load(&state, &owner, &slug, user.as_ref().map(|u| u.id), Role::Viewer).await?;
    let rows: Vec<Issue> = sqlx::query_as(&format!("SELECT {COLUMNS} {FROM} WHERE i.project_id = $1 ORDER BY i.number")).bind(p.id).fetch_all(&state.db).await?;
    let mut topics = Vec::with_capacity(rows.len());
    for i in rows {
        let comments: Vec<(String, String, DateTime<Utc>)> = sqlx::query_as(
            "SELECT a.slug, c.body, c.created_at FROM comments c JOIN accounts a ON a.id = c.author_id WHERE c.issue_id = $1 ORDER BY c.created_at",
        )
        .bind(i.id)
        .fetch_all(&state.db)
        .await?;
        let mut vp = i.viewpoint.clone();
        if vp.is_none() && !i.elements.is_empty() {
            vp = Some(json!({"selected": i.elements}));
        }
        topics.push(bcf::Topic {
            guid: i.id.to_string(),
            title: i.title,
            description: i.body,
            status: if i.state == "open" { "Open".into() } else { "Closed".into() },
            priority: i.priority,
            author: i.author,
            assignee: i.assignee,
            created: i.created_at.to_rfc3339(),
            comments: comments.into_iter().map(|(a, b, at)| (a, at.to_rfc3339(), b)).collect(),
            viewpoint: vp,
        });
    }
    let bytes = bcf::export(&p.full_name(), &topics)?;
    if let Some(u) = &user {
        crate::audit::record(&state.db, Some(p.owner_id), Some(u.id), "issues.export", &p.full_name(), json!({"count": topics.len()})).await?;
    }
    Ok((
        [
            (header::CONTENT_TYPE, "application/octet-stream".to_string()),
            (header::CONTENT_DISPOSITION, format!("attachment; filename=\"{}-issues.bcfzip\"", p.slug)),
        ],
        bytes,
    )
        .into_response())
}

const MAX_BCF: usize = 64 << 20;

async fn import(State(state): State<AppState>, user: CurrentUser, Path((owner, slug)): Path<(String, String)>, mut form: Multipart) -> ApiResult<Json<Value>> {
    let (p, _) = load(&state, &owner, &slug, Some(user.id), Role::Reviewer).await?;
    let mut bytes = None;
    while let Some(field) = form.next_field().await.map_err(|e| AppError::bad_request(e.to_string()))? {
        if field.name() == Some("file") {
            let b = field.bytes().await.map_err(|e| AppError::bad_request(e.to_string()))?;
            if b.len() > MAX_BCF {
                return Err(AppError::new(axum::http::StatusCode::PAYLOAD_TOO_LARGE, "too_large", "BCF files are limited to 64 MB"));
            }
            bytes = Some(b);
        }
    }
    let bytes = bytes.ok_or_else(|| AppError::bad_request("attach a .bcfzip file"))?;
    let topics = bcf::import(&bytes, 8 << 20).map_err(|_| AppError::bad_request("this is not a BCF file"))?;
    let mut created = Vec::new();
    for t in topics {
        let elements: Vec<String> = t
            .viewpoint
            .as_ref()
            .and_then(|v| v["selected"].as_array())
            .map(|a| a.iter().filter_map(|g| g.as_str().map(str::to_string)).collect())
            .unwrap_or_default();
        let state_str = if t.status.eq_ignore_ascii_case("closed") || t.status.eq_ignore_ascii_case("resolved") { "closed" } else { "open" };
        let priority = t.priority.to_lowercase();
        let priority = if PRIORITIES.contains(&priority.as_str()) { priority } else { "normal".to_string() };
        let mut body = t.description.clone();
        if !t.author.is_empty() {
            body = format!("{body}\n\n(Imported from BCF; created by {})", t.author).trim().to_string();
        }
        let (id, number) = insert(
            &state,
            &p,
            Insert {
                title: &t.title,
                body: &body,
                state: state_str,
                priority: &priority,
                author: user.id,
                assignee: None,
                labels: &[],
                viewpoint: t.viewpoint.as_ref(),
                elements: &elements,
                version: None,
            },
        )
        .await?;
        for (author, _, text) in &t.comments {
            let text = if author.is_empty() { text.clone() } else { format!("{author}: {text}") };
            sqlx::query("INSERT INTO comments (id, project_id, issue_id, author_id, body) VALUES ($1, $2, $3, $4, $5)")
                .bind(Uuid::new_v4())
                .bind(p.id)
                .bind(id)
                .bind(user.id)
                .bind(text)
                .execute(&state.db)
                .await?;
        }
        created.push(number);
    }
    crate::activity::record(&state.db, p.id, Some(user.id), "issues.imported", json!({"count": created.len()})).await?;
    Ok(Json(json!({"created": created})))
}
