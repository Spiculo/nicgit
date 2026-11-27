// SPDX-License-Identifier: AGPL-3.0-or-later
use crate::{
    app::AppState,
    audit,
    auth::{CurrentUser, MaybeUser},
    error::{ApiResult, AppError},
};
use axum::{
    extract::{Path, State},
    routing::{get, post, put},
    Json, Router,
};
use nicgit_core::roles::Role;
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/orgs", post(create_org))
        .route("/api/orgs/{slug}", get(get_org))
        .route("/api/orgs/{slug}/members/{user}", put(set_member).delete(remove_member))
        .route("/api/orgs/{slug}/projects", get(org_projects))
        .route("/api/orgs/{slug}/teams", get(list_teams).post(create_team))
        .route("/api/orgs/{slug}/teams/{team}", get(get_team).delete(delete_team))
        .route("/api/orgs/{slug}/teams/{team}/members/{user}", put(add_team_member).delete(remove_team_member))
        .route("/api/orgs/{slug}/teams/{team}/projects/{project}", put(set_team_project).delete(remove_team_project))
        .route("/api/accounts/{slug}", get(get_account))
}

/// An organisation's id and the caller's role in it.
pub async fn org_role(state: &AppState, slug: &str, user: Option<Uuid>) -> ApiResult<(Uuid, Option<Role>)> {
    let id: Uuid = sqlx::query_scalar("SELECT id FROM accounts WHERE slug = $1 AND kind = 'org'")
        .bind(slug)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(AppError::not_found)?;
    let role: Option<String> = match user {
        Some(u) => {
            sqlx::query_scalar("SELECT role FROM org_members WHERE org_id = $1 AND user_id = $2")
                .bind(id)
                .bind(u)
                .fetch_optional(&state.db)
                .await?
        }
        None => None,
    };
    Ok((id, role.and_then(|r| r.parse().ok())))
}

pub async fn user_id(state: &AppState, slug: &str) -> ApiResult<Uuid> {
    sqlx::query_scalar("SELECT id FROM accounts WHERE slug = $1 AND kind = 'user'")
        .bind(slug)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(AppError::not_found)
}

#[derive(Deserialize)]
struct NewOrg {
    slug: String,
    #[serde(default)]
    display_name: String,
}

async fn create_org(State(state): State<AppState>, user: CurrentUser, Json(b): Json<NewOrg>) -> ApiResult<Json<Value>> {
    if !nicgit_core::names::valid_slug(&b.slug) {
        return Err(AppError::bad_request("organisation names use lowercase letters, digits, '-', '_' or '.'"));
    }
    crate::limits::check_new_org(&state, user.id).await?;
    let id = Uuid::new_v4();
    let display = if b.display_name.trim().is_empty() { b.slug.clone() } else { b.display_name.trim().to_string() };
    let mut tx = state.db.begin().await?;
    let r = sqlx::query(
        "INSERT INTO accounts (id, kind, slug, display_name) SELECT $1, 'org', $2, $3 \
         WHERE NOT EXISTS (SELECT 1 FROM retired_slugs WHERE slug = $2) ON CONFLICT DO NOTHING",
    )
        .bind(id)
        .bind(&b.slug)
        .bind(&display)
        .execute(&mut *tx)
        .await?;
    if r.rows_affected() == 0 {
        return Err(AppError::conflict("taken", "that name is already in use"));
    }
    sqlx::query("INSERT INTO org_members (org_id, user_id, role) VALUES ($1, $2, 'admin')")
        .bind(id)
        .bind(user.id)
        .execute(&mut *tx)
        .await?;
    audit::record(&mut *tx, Some(id), Some(user.id), "org.create", &b.slug, json!({})).await?;
    tx.commit().await?;
    Ok(Json(json!({"id": id, "slug": b.slug})))
}

async fn get_org(State(state): State<AppState>, MaybeUser(user): MaybeUser, Path(slug): Path<String>) -> ApiResult<Json<Value>> {
    let (id, role) = org_role(&state, &slug, user.as_ref().map(|u| u.id)).await?;
    let display_name: String =
        sqlx::query_scalar("SELECT display_name FROM accounts WHERE id = $1").bind(id).fetch_one(&state.db).await?;
    let mut out = json!({"id": id, "slug": slug, "display_name": display_name, "kind": "org", "role": role});
    if role.is_some() {
        let members: Vec<(String, String)> = sqlx::query_as(
            "SELECT a.slug, m.role FROM org_members m JOIN accounts a ON a.id = m.user_id WHERE m.org_id = $1 ORDER BY a.slug",
        )
        .bind(id)
        .fetch_all(&state.db)
        .await?;
        out["members"] = json!(members.into_iter().map(|(s, r)| json!({"slug": s, "role": r})).collect::<Vec<_>>());
    }
    Ok(Json(out))
}

#[derive(Deserialize)]
struct MemberRole {
    role: String,
}

async fn set_member(
    State(state): State<AppState>,
    user: CurrentUser,
    Path((slug, member)): Path<(String, String)>,
    Json(b): Json<MemberRole>,
) -> ApiResult<Json<Value>> {
    let (org, role) = org_role(&state, &slug, Some(user.id)).await?;
    if role != Some(Role::Admin) {
        return Err(AppError::forbidden("only organisation admins manage members"));
    }
    let new_role: Role = b.role.parse().map_err(|_| AppError::bad_request("unknown role"))?;
    let member_id = user_id(&state, &member).await?;
    sqlx::query(
        "INSERT INTO org_members (org_id, user_id, role) VALUES ($1, $2, $3) \
         ON CONFLICT (org_id, user_id) DO UPDATE SET role = $3",
    )
    .bind(org)
    .bind(member_id)
    .bind(new_role.as_str())
    .execute(&state.db)
    .await?;
    audit::record(&state.db, Some(org), Some(user.id), "org.member_set", &member, json!({"role": new_role})).await?;
    Ok(Json(json!({"ok": true})))
}

async fn remove_member(
    State(state): State<AppState>,
    user: CurrentUser,
    Path((slug, member)): Path<(String, String)>,
) -> ApiResult<Json<Value>> {
    let (org, role) = org_role(&state, &slug, Some(user.id)).await?;
    if role.is_none() {
        return Err(AppError::not_found());
    }
    if role != Some(Role::Admin) && member != user.slug {
        return Err(AppError::forbidden("only organisation admins manage members"));
    }
    let member_id = user_id(&state, &member).await?;
    let other_admins: i64 =
        sqlx::query_scalar("SELECT count(*) FROM org_members WHERE org_id = $1 AND role = 'admin' AND user_id <> $2")
            .bind(org)
            .bind(member_id)
            .fetch_one(&state.db)
            .await?;
    if other_admins == 0 {
        return Err(AppError::conflict("last_admin", "an organisation needs at least one admin"));
    }
    sqlx::query("DELETE FROM org_members WHERE org_id = $1 AND user_id = $2").bind(org).bind(member_id).execute(&state.db).await?;
    audit::record(&state.db, Some(org), Some(user.id), "org.member_remove", &member, json!({})).await?;
    Ok(Json(json!({"ok": true})))
}

async fn org_projects(State(state): State<AppState>, MaybeUser(user): MaybeUser, Path(slug): Path<String>) -> ApiResult<Json<Value>> {
    let (org, role) = org_role(&state, &slug, user.as_ref().map(|u| u.id)).await?;
    if role.is_none() {
        return Err(AppError::not_found());
    }
    let rows: Vec<(Uuid, String, String, String, String, String)> = sqlx::query_as(
        "SELECT id, slug, description, visibility, default_option, created_at::text FROM projects \
         WHERE owner_id = $1 ORDER BY created_at DESC",
    )
    .bind(org)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(json!(rows
        .into_iter()
        .map(|(id, project, description, visibility, default_option, created_at)| json!({
            "id": id, "owner": slug, "owner_kind": "org", "slug": project,
            "full_name": format!("{slug}/{project}"),
            "description": description, "visibility": visibility, "default_option": default_option,
            "created_at": created_at, "role": role,
        }))
        .collect::<Vec<_>>())))
}

async fn list_teams(State(state): State<AppState>, MaybeUser(user): MaybeUser, Path(slug): Path<String>) -> ApiResult<Json<Value>> {
    let (org, role) = org_role(&state, &slug, user.as_ref().map(|u| u.id)).await?;
    if role.is_none() {
        return Err(AppError::not_found());
    }
    let rows: Vec<(Uuid, String, String, i64, i64)> = sqlx::query_as(
        "SELECT t.id, t.slug, t.name,            (SELECT count(*) FROM team_members m WHERE m.team_id = t.id),            (SELECT count(*) FROM team_project_access a WHERE a.team_id = t.id)          FROM teams t WHERE t.org_id = $1 ORDER BY t.slug",
    )
    .bind(org)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(json!(rows
        .into_iter()
        .map(|(id, slug, name, members, projects)| json!({"id": id, "slug": slug, "name": name, "members": members, "projects": projects}))
        .collect::<Vec<_>>())))
}

#[derive(Deserialize)]
struct NewTeam {
    slug: String,
    #[serde(default)]
    name: String,
}

async fn create_team(State(state): State<AppState>, user: CurrentUser, Path(slug): Path<String>, Json(b): Json<NewTeam>) -> ApiResult<Json<Value>> {
    let (org, role) = org_role(&state, &slug, Some(user.id)).await?;
    if role != Some(Role::Admin) {
        return Err(AppError::forbidden("only organisation admins manage teams"));
    }
    if !nicgit_core::names::valid_slug(&b.slug) {
        return Err(AppError::bad_request("team names use lowercase letters, digits, '-', '_' or '.'"));
    }
    let id = Uuid::new_v4();
    let name = if b.name.trim().is_empty() { b.slug.clone() } else { b.name.trim().to_string() };
    sqlx::query("INSERT INTO teams (id, org_id, slug, name) VALUES ($1, $2, $3, $4)")
        .bind(id)
        .bind(org)
        .bind(&b.slug)
        .bind(&name)
        .execute(&state.db)
        .await
        .map_err(|_| AppError::conflict("taken", "that team name is already in use"))?;
    audit::record(&state.db, Some(org), Some(user.id), "org.team_create", &b.slug, json!({})).await?;
    Ok(Json(json!({"id": id, "slug": b.slug, "name": name})))
}

async fn get_team(State(state): State<AppState>, MaybeUser(user): MaybeUser, Path((slug, team)): Path<(String, String)>) -> ApiResult<Json<Value>> {
    let (org, role) = org_role(&state, &slug, user.as_ref().map(|u| u.id)).await?;
    if role.is_none() {
        return Err(AppError::not_found());
    }
    let row: Option<(Uuid, String)> = sqlx::query_as("SELECT id, name FROM teams WHERE org_id = $1 AND slug = $2")
        .bind(org)
        .bind(&team)
        .fetch_optional(&state.db)
        .await?;
    let (id, name) = row.ok_or_else(AppError::not_found)?;
    let members: Vec<String> = sqlx::query_scalar(
        "SELECT a.slug FROM team_members m JOIN accounts a ON a.id = m.user_id WHERE m.team_id = $1 ORDER BY a.slug",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;
    let projects: Vec<(String, String)> = sqlx::query_as(
        "SELECT p.slug, a.role FROM team_project_access a JOIN projects p ON p.id = a.project_id          WHERE a.team_id = $1 ORDER BY p.slug",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(json!({"id": id, "slug": team, "name": name, "members": members,
        "projects": projects.into_iter().map(|(p, r)| json!({"slug": p, "role": r})).collect::<Vec<_>>()})))
}

async fn delete_team(State(state): State<AppState>, user: CurrentUser, Path((slug, team)): Path<(String, String)>) -> ApiResult<Json<Value>> {
    let (org, role) = org_role(&state, &slug, Some(user.id)).await?;
    if role != Some(Role::Admin) {
        return Err(AppError::forbidden("only organisation admins manage teams"));
    }
    let n = sqlx::query("DELETE FROM teams WHERE org_id = $1 AND slug = $2").bind(org).bind(&team).execute(&state.db).await?;
    if n.rows_affected() == 0 {
        return Err(AppError::not_found());
    }
    audit::record(&state.db, Some(org), Some(user.id), "org.team_delete", &team, json!({})).await?;
    Ok(Json(json!({"ok": true})))
}

async fn team_id(state: &AppState, org: Uuid, team: &str) -> ApiResult<Uuid> {
    sqlx::query_scalar("SELECT id FROM teams WHERE org_id = $1 AND slug = $2")
        .bind(org)
        .bind(team)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(AppError::not_found)
}

async fn add_team_member(State(state): State<AppState>, user: CurrentUser, Path((slug, team, member)): Path<(String, String, String)>) -> ApiResult<Json<Value>> {
    let (org, role) = org_role(&state, &slug, Some(user.id)).await?;
    if role != Some(Role::Admin) {
        return Err(AppError::forbidden("only organisation admins manage teams"));
    }
    let team_id = team_id(&state, org, &team).await?;
    let member_id = user_id(&state, &member).await?;
    let in_org: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM org_members WHERE org_id = $1 AND user_id = $2)")
        .bind(org)
        .bind(member_id)
        .fetch_one(&state.db)
        .await?;
    if !in_org {
        return Err(AppError::bad_request("add them to the organisation first"));
    }
    sqlx::query("INSERT INTO team_members (team_id, user_id) VALUES ($1, $2) ON CONFLICT DO NOTHING")
        .bind(team_id)
        .bind(member_id)
        .execute(&state.db)
        .await?;
    audit::record(&state.db, Some(org), Some(user.id), "org.team_member_add", &team, json!({"user": member})).await?;
    Ok(Json(json!({"ok": true})))
}

async fn remove_team_member(State(state): State<AppState>, user: CurrentUser, Path((slug, team, member)): Path<(String, String, String)>) -> ApiResult<Json<Value>> {
    let (org, role) = org_role(&state, &slug, Some(user.id)).await?;
    if role != Some(Role::Admin) {
        return Err(AppError::forbidden("only organisation admins manage teams"));
    }
    let team_id = team_id(&state, org, &team).await?;
    let member_id = user_id(&state, &member).await?;
    sqlx::query("DELETE FROM team_members WHERE team_id = $1 AND user_id = $2").bind(team_id).bind(member_id).execute(&state.db).await?;
    Ok(Json(json!({"ok": true})))
}

#[derive(Deserialize)]
struct TeamRole {
    role: String,
}

async fn set_team_project(State(state): State<AppState>, user: CurrentUser, Path((slug, team, project)): Path<(String, String, String)>, Json(b): Json<TeamRole>) -> ApiResult<Json<Value>> {
    let (org, role) = org_role(&state, &slug, Some(user.id)).await?;
    if role != Some(Role::Admin) {
        return Err(AppError::forbidden("only organisation admins manage teams"));
    }
    let team_id = team_id(&state, org, &team).await?;
    let project_id: Uuid = sqlx::query_scalar("SELECT id FROM projects WHERE owner_id = $1 AND slug = $2")
        .bind(org)
        .bind(&project)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(AppError::not_found)?;
    let new_role: Role = b.role.parse().map_err(|_| AppError::bad_request("unknown role"))?;
    sqlx::query("INSERT INTO team_project_access (team_id, project_id, role) VALUES ($1, $2, $3)                  ON CONFLICT (team_id, project_id) DO UPDATE SET role = $3")
        .bind(team_id)
        .bind(project_id)
        .bind(new_role.as_str())
        .execute(&state.db)
        .await?;
    Ok(Json(json!({"ok": true})))
}

async fn remove_team_project(State(state): State<AppState>, user: CurrentUser, Path((slug, team, project)): Path<(String, String, String)>) -> ApiResult<Json<Value>> {
    let (org, role) = org_role(&state, &slug, Some(user.id)).await?;
    if role != Some(Role::Admin) {
        return Err(AppError::forbidden("only organisation admins manage teams"));
    }
    let team_id = team_id(&state, org, &team).await?;
    let project_id: Uuid = sqlx::query_scalar("SELECT id FROM projects WHERE owner_id = $1 AND slug = $2")
        .bind(org)
        .bind(&project)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(AppError::not_found)?;
    sqlx::query("DELETE FROM team_project_access WHERE team_id = $1 AND project_id = $2").bind(team_id).bind(project_id).execute(&state.db).await?;
    Ok(Json(json!({"ok": true})))
}

async fn get_account(State(state): State<AppState>, Path(slug): Path<String>) -> ApiResult<Json<Value>> {
    let row: Option<(Uuid, String, String)> =
        sqlx::query_as("SELECT id, kind, display_name FROM accounts WHERE slug = $1").bind(&slug).fetch_optional(&state.db).await?;
    let (id, kind, display_name) = row.ok_or_else(AppError::not_found)?;
    Ok(Json(json!({"id": id, "slug": slug, "kind": kind, "display_name": display_name})))
}
