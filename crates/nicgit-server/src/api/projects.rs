// SPDX-License-Identifier: AGPL-3.0-or-later
use crate::{
    access::{self, Project, PROJECT_COLUMNS},
    api::orgs::{org_role, user_id},
    app::AppState,
    audit,
    auth::{CurrentUser, MaybeUser},
    error::{ApiResult, AppError},
    repos,
};
use axum::{
    extract::{Path, Query, State},
    routing::{get, post, put},
    Json, Router,
};
use chrono::{DateTime, Utc};
use nicgit_core::roles::Role;
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/projects", post(create).get(list_mine))
        .route("/api/explore", get(explore))
        .route("/api/projects/{owner}/{slug}", get(get_one).patch(update).delete(remove))
        .route("/api/projects/{owner}/{slug}/members", get(members))
        .route("/api/projects/{owner}/{slug}/members/{user}", put(set_member).delete(remove_member))
        .route("/api/projects/{owner}/{slug}/activity", get(activity))
}

#[derive(Deserialize)]
pub struct NewProject {
    pub owner: String,
    pub slug: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "private")]
    pub visibility: String,
}

fn private() -> String {
    "private".into()
}

fn check_visibility(v: &str) -> ApiResult<()> {
    if v == "private" || v == "public" {
        Ok(())
    } else {
        Err(AppError::bad_request("visibility is private or public"))
    }
}

/// The account a new project would belong to, when this user may create projects for it.
pub async fn new_project_owner(state: &AppState, user: &CurrentUser, slug: &str) -> ApiResult<Uuid> {
    let owner: Option<(Uuid, String)> = sqlx::query_as("SELECT id, kind FROM accounts WHERE slug = $1").bind(slug).fetch_optional(&state.db).await?;
    let (owner_id, kind) = owner.ok_or_else(AppError::not_found)?;
    let allowed = if kind == "user" {
        owner_id == user.id
    } else {
        org_role(state, slug, Some(user.id)).await?.1.is_some_and(|r| r >= Role::Lead)
    };
    if !allowed {
        return Err(AppError::forbidden("you cannot create projects for this account"));
    }
    Ok(owner_id)
}

/// Create a project row and its repository. Shared with imports (uploads).
pub async fn create_project(state: &AppState, user: &CurrentUser, b: &NewProject) -> ApiResult<Project> {
    if !nicgit_core::names::valid_slug(&b.slug) {
        return Err(AppError::bad_request("project names use lowercase letters, digits, '-', '_' or '.'"));
    }
    check_visibility(&b.visibility)?;
    let owner_id = new_project_owner(state, user, &b.owner).await?;
    crate::limits::check_new_project(state, owner_id).await?;
    let id = Uuid::new_v4();
    let r = sqlx::query(
        "INSERT INTO projects (id, owner_id, slug, description, visibility) VALUES ($1, $2, $3, $4, $5) ON CONFLICT DO NOTHING",
    )
    .bind(id)
    .bind(owner_id)
    .bind(&b.slug)
    .bind(b.description.trim())
    .bind(&b.visibility)
    .execute(&state.db)
    .await?;
    if r.rows_affected() == 0 {
        return Err(AppError::conflict("taken", "a project with that name already exists"));
    }
    if let Err(e) = repos::init(&state.cfg, id).await {
        sqlx::query("DELETE FROM projects WHERE id = $1").bind(id).execute(&state.db).await?;
        return Err(AppError::internal(e));
    }
    audit::record(&state.db, Some(owner_id), Some(user.id), "project.create", &format!("{}/{}", b.owner, b.slug), json!({"visibility": b.visibility}))
        .await?;
    crate::activity::record(&state.db, id, Some(user.id), "project.created", json!({})).await?;
    Ok(access::find(state, &b.owner, &b.slug).await?.expect("just created"))
}

async fn create(State(state): State<AppState>, user: CurrentUser, Json(b): Json<NewProject>) -> ApiResult<Json<Value>> {
    let p = create_project(&state, &user, &b).await?;
    Ok(Json(p.to_json(Some(Role::Admin))))
}

/// Projects the caller can reach through ownership, organisations, grants or teams.
async fn list_mine(State(state): State<AppState>, user: CurrentUser) -> ApiResult<Json<Value>> {
    let rows: Vec<Project> = sqlx::query_as(&format!(
        "SELECT {PROJECT_COLUMNS} FROM projects p JOIN accounts a ON a.id = p.owner_id WHERE \
           (a.kind = 'user' AND p.owner_id = $1) \
           OR p.owner_id IN (SELECT org_id FROM org_members WHERE user_id = $1) \
           OR p.id IN (SELECT project_id FROM project_members WHERE user_id = $1) \
           OR p.id IN (SELECT tpa.project_id FROM team_project_access tpa JOIN team_members tm ON tm.team_id = tpa.team_id WHERE tm.user_id = $1) \
         ORDER BY a.slug, p.slug"
    ))
    .bind(user.id)
    .fetch_all(&state.db)
    .await?;
    let mut out = Vec::with_capacity(rows.len());
    for p in rows {
        let role = access::project_role(&state, Some(user.id), &p).await?;
        out.push(p.to_json(role));
    }
    Ok(Json(json!(out)))
}

async fn explore(State(state): State<AppState>) -> ApiResult<Json<Value>> {
    let rows: Vec<Project> = sqlx::query_as(&format!(
        "SELECT {PROJECT_COLUMNS} FROM projects p JOIN accounts a ON a.id = p.owner_id WHERE p.visibility = 'public' \
         ORDER BY p.created_at DESC LIMIT 100"
    ))
    .fetch_all(&state.db)
    .await?;
    Ok(Json(json!(rows.iter().map(|p| p.to_json(Some(Role::Viewer))).collect::<Vec<_>>())))
}

async fn get_one(State(state): State<AppState>, MaybeUser(user): MaybeUser, Path((owner, slug)): Path<(String, String)>) -> ApiResult<Json<Value>> {
    let (p, role) = access::load(&state, &owner, &slug, user.map(|u| u.id), Role::Viewer).await?;
    Ok(Json(p.to_json(Some(role))))
}

#[derive(Deserialize)]
struct ProjectUpdate {
    description: Option<String>,
    visibility: Option<String>,
    default_option: Option<String>,
}

async fn update(
    State(state): State<AppState>,
    user: CurrentUser,
    Path((owner, slug)): Path<(String, String)>,
    Json(b): Json<ProjectUpdate>,
) -> ApiResult<Json<Value>> {
    let (p, _) = access::load(&state, &owner, &slug, Some(user.id), Role::Admin).await?;
    if let Some(v) = &b.visibility {
        check_visibility(v)?;
        if *v != p.visibility {
            sqlx::query("UPDATE projects SET visibility = $2 WHERE id = $1").bind(p.id).bind(v).execute(&state.db).await?;
            audit::record(&state.db, Some(p.owner_id), Some(user.id), "project.visibility", &p.full_name(), json!({"from": p.visibility, "to": v}))
                .await?;
        }
    }
    if let Some(d) = &b.description {
        sqlx::query("UPDATE projects SET description = $2 WHERE id = $1").bind(p.id).bind(d.trim()).execute(&state.db).await?;
    }
    if let Some(o) = &b.default_option {
        if o.is_empty() || o.contains("..") || o.starts_with('/') {
            return Err(AppError::bad_request("not a design option name"));
        }
        let git = crate::gitread::Git::new(&p.repo_dir(&state));
        if git.resolve(o).await.is_err() {
            return Err(AppError::bad_request("that design option does not exist"));
        }
        sqlx::query("UPDATE projects SET default_option = $2 WHERE id = $1").bind(p.id).bind(o).execute(&state.db).await?;
        crate::repos::set_head(&state.cfg, p.id, o).await.map_err(AppError::internal)?;
    }
    let p = access::find(&state, &owner, &slug).await?.ok_or_else(AppError::not_found)?;
    Ok(Json(p.to_json(Some(Role::Admin))))
}

async fn remove(State(state): State<AppState>, user: CurrentUser, Path((owner, slug)): Path<(String, String)>) -> ApiResult<Json<Value>> {
    let (p, _) = access::load(&state, &owner, &slug, Some(user.id), Role::Admin).await?;
    sqlx::query("DELETE FROM projects WHERE id = $1").bind(p.id).execute(&state.db).await?;
    repos::remove_all(&p.repo_dir(&state))?;
    repos::remove_all(&repos::cache_dir(&state.cfg, p.id))?;
    repos::remove_all(&repos::uploads_dir(&state.cfg, p.id))?;
    audit::record(&state.db, Some(p.owner_id), Some(user.id), "project.delete", &p.full_name(), json!({})).await?;
    Ok(Json(json!({"ok": true})))
}

async fn members(State(state): State<AppState>, MaybeUser(user): MaybeUser, Path((owner, slug)): Path<(String, String)>) -> ApiResult<Json<Value>> {
    let (p, _) = access::load(&state, &owner, &slug, user.map(|u| u.id), Role::Viewer).await?;
    let direct: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT a.slug, a.display_name, m.role FROM project_members m JOIN accounts a ON a.id = m.user_id \
         WHERE m.project_id = $1 ORDER BY a.slug",
    )
    .bind(p.id)
    .fetch_all(&state.db)
    .await?;
    let mut out: Vec<Value> =
        direct.into_iter().map(|(s, n, r)| json!({"slug": s, "display_name": n, "role": r, "via": "project"})).collect();
    if p.owner_kind == "org" {
        let org: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT a.slug, a.display_name, m.role FROM org_members m JOIN accounts a ON a.id = m.user_id \
             WHERE m.org_id = $1 ORDER BY a.slug",
        )
        .bind(p.owner_id)
        .fetch_all(&state.db)
        .await?;
        out.extend(org.into_iter().map(|(s, n, r)| json!({"slug": s, "display_name": n, "role": r, "via": "organisation"})));
    } else {
        out.push(json!({"slug": p.owner_slug, "role": "admin", "via": "owner"}));
    }
    Ok(Json(json!(out)))
}

#[derive(Deserialize)]
struct MemberRole {
    role: String,
}

async fn set_member(
    State(state): State<AppState>,
    user: CurrentUser,
    Path((owner, slug, member)): Path<(String, String, String)>,
    Json(b): Json<MemberRole>,
) -> ApiResult<Json<Value>> {
    let (p, _) = access::load(&state, &owner, &slug, Some(user.id), Role::Admin).await?;
    let role: Role = b.role.parse().map_err(|_| AppError::bad_request("unknown role"))?;
    let member_id = user_id(&state, &member).await?;
    sqlx::query(
        "INSERT INTO project_members (project_id, user_id, role) VALUES ($1, $2, $3) \
         ON CONFLICT (project_id, user_id) DO UPDATE SET role = $3",
    )
    .bind(p.id)
    .bind(member_id)
    .bind(role.as_str())
    .execute(&state.db)
    .await?;
    audit::record(&state.db, Some(p.owner_id), Some(user.id), "project.member_set", &format!("{}:{member}", p.full_name()), json!({"role": role}))
        .await?;
    crate::notify::send(&state.db, member_id, "project.added", json!({"project": p.full_name(), "role": role})).await?;
    Ok(Json(json!({"ok": true})))
}

async fn remove_member(
    State(state): State<AppState>,
    user: CurrentUser,
    Path((owner, slug, member)): Path<(String, String, String)>,
) -> ApiResult<Json<Value>> {
    let (p, _) = access::load(&state, &owner, &slug, Some(user.id), Role::Admin).await?;
    let member_id = user_id(&state, &member).await?;
    sqlx::query("DELETE FROM project_members WHERE project_id = $1 AND user_id = $2").bind(p.id).bind(member_id).execute(&state.db).await?;
    audit::record(&state.db, Some(p.owner_id), Some(user.id), "project.member_remove", &format!("{}:{member}", p.full_name()), json!({}))
        .await?;
    Ok(Json(json!({"ok": true})))
}

#[derive(Deserialize)]
struct Page {
    before: Option<i64>,
}

async fn activity(
    State(state): State<AppState>,
    MaybeUser(user): MaybeUser,
    Path((owner, slug)): Path<(String, String)>,
    Query(q): Query<Page>,
) -> ApiResult<Json<Value>> {
    let (p, _) = access::load(&state, &owner, &slug, user.map(|u| u.id), Role::Viewer).await?;
    let rows: Vec<(i64, Option<String>, String, Value, DateTime<Utc>)> = sqlx::query_as(
        "SELECT x.id, a.slug, x.kind, x.data, x.created_at FROM activity x LEFT JOIN accounts a ON a.id = x.actor_id \
         WHERE x.project_id = $1 AND ($2::bigint IS NULL OR x.id < $2) ORDER BY x.id DESC LIMIT 50",
    )
    .bind(p.id)
    .bind(q.before)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(json!(rows
        .into_iter()
        .map(|(id, actor, kind, data, at)| json!({"id": id, "actor": actor, "kind": kind, "data": data, "created_at": at}))
        .collect::<Vec<_>>())))
}
