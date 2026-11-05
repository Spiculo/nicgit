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

async fn get_account(State(state): State<AppState>, Path(slug): Path<String>) -> ApiResult<Json<Value>> {
    let row: Option<(Uuid, String, String)> =
        sqlx::query_as("SELECT id, kind, display_name FROM accounts WHERE slug = $1").bind(&slug).fetch_optional(&state.db).await?;
    let (id, kind, display_name) = row.ok_or_else(AppError::not_found)?;
    Ok(Json(json!({"id": id, "slug": slug, "kind": kind, "display_name": display_name})))
}
