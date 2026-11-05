// SPDX-License-Identifier: AGPL-3.0-or-later
//! Account-level settings an admin manages: usage, budgets (spending caps) and AI.
use crate::{
    app::AppState,
    audit,
    auth::CurrentUser,
    error::{ApiResult, AppError},
    metering,
};
use axum::{
    extract::{Path, State},
    routing::{get, put},
    Json, Router,
};
use nicgit_core::roles::Role;
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/accounts/{slug}/usage", get(usage))
        .route("/api/accounts/{slug}/ai", get(get_ai).put(set_ai))
        .route("/api/accounts/{slug}/budgets/{kind}", put(set_budget))
        .route("/api/me/usage", get(my_usage))
        .route("/api/me/storage", get(my_storage))
}

async fn my_storage(State(state): State<AppState>, user: CurrentUser) -> ApiResult<Json<Value>> {
    Ok(Json(crate::limits::summary(&state, user.id).await?))
}

/// The account's id when the caller administers it (themselves, or an organisation they are admin of).
/// Anyone else is told it does not exist.
pub async fn administered(state: &AppState, slug: &str, user: &CurrentUser) -> ApiResult<Uuid> {
    let row: Option<(Uuid, String)> = sqlx::query_as("SELECT id, kind FROM accounts WHERE slug = $1").bind(slug).fetch_optional(&state.db).await?;
    let (id, kind) = row.ok_or_else(AppError::not_found)?;
    let ok = if kind == "user" {
        id == user.id
    } else {
        crate::api::orgs::org_role(state, slug, Some(user.id)).await?.1 == Some(Role::Admin)
    };
    if ok { Ok(id) } else { Err(AppError::not_found()) }
}

async fn usage_of(state: &AppState, account: Uuid) -> ApiResult<Value> {
    let usage: Vec<Value> = metering::usage(state, account).await?.into_iter().map(|(k, q)| json!({"kind": k, "quantity": q})).collect();
    Ok(json!({"usage": usage, "budgets": metering::budgets(state, account).await?, "storage": crate::limits::summary(state, account).await?}))
}

async fn usage(State(state): State<AppState>, user: CurrentUser, Path(slug): Path<String>) -> ApiResult<Json<Value>> {
    let id = administered(&state, &slug, &user).await?;
    Ok(Json(usage_of(&state, id).await?))
}

async fn my_usage(State(state): State<AppState>, user: CurrentUser) -> ApiResult<Json<Value>> {
    Ok(Json(usage_of(&state, user.id).await?["usage"].clone()))
}

async fn get_ai(State(state): State<AppState>, user: CurrentUser, Path(slug): Path<String>) -> ApiResult<Json<Value>> {
    let id = administered(&state, &slug, &user).await?;
    Ok(Json(json!({"enabled": crate::ai::enabled(&state, id).await?, "router": state.cfg.ai_router_url.is_some()})))
}

#[derive(Deserialize)]
struct AiBody {
    enabled: bool,
}

async fn set_ai(State(state): State<AppState>, user: CurrentUser, Path(slug): Path<String>, Json(b): Json<AiBody>) -> ApiResult<Json<Value>> {
    let id = administered(&state, &slug, &user).await?;
    sqlx::query("INSERT INTO ai_settings (account_id, enabled) VALUES ($1, $2) ON CONFLICT (account_id) DO UPDATE SET enabled = $2")
        .bind(id)
        .bind(b.enabled)
        .execute(&state.db)
        .await?;
    audit::record(&state.db, Some(id), Some(user.id), "ai.settings", &slug, json!({"enabled": b.enabled})).await?;
    Ok(Json(json!({"enabled": b.enabled})))
}

#[derive(Deserialize)]
struct BudgetBody {
    soft_limit: Option<i64>,
    hard_limit: Option<i64>,
}

async fn set_budget(State(state): State<AppState>, user: CurrentUser, Path((slug, kind)): Path<(String, String)>, Json(b): Json<BudgetBody>) -> ApiResult<Json<Value>> {
    let id = administered(&state, &slug, &user).await?;
    if !metering::KINDS.contains(&kind.as_str()) {
        return Err(AppError::bad_request("unknown usage kind"));
    }
    if b.soft_limit.is_some_and(|v| v < 0) || b.hard_limit.is_some_and(|v| v < 0) {
        return Err(AppError::bad_request("limits cannot be negative"));
    }
    sqlx::query(
        "INSERT INTO budgets (account_id, kind, soft_limit, hard_limit) VALUES ($1, $2, $3, $4) \
         ON CONFLICT (account_id, kind) DO UPDATE SET soft_limit = $3, hard_limit = $4",
    )
    .bind(id)
    .bind(&kind)
    .bind(b.soft_limit)
    .bind(b.hard_limit)
    .execute(&state.db)
    .await?;
    audit::record(&state.db, Some(id), Some(user.id), "budget.set", &format!("{slug}:{kind}"), json!({"soft": b.soft_limit, "hard": b.hard_limit})).await?;
    Ok(Json(json!({"ok": true})))
}
