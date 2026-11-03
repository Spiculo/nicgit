// SPDX-License-Identifier: AGPL-3.0-or-later
//! Protected design options and their rules.
use crate::{
    access,
    api::versions::{json_file, load},
    app::AppState,
    audit,
    auth::{CurrentUser, MaybeUser},
    error::{ApiResult, AppError},
    rules::OptionRule,
};
use axum::{
    extract::{Path, State},
    routing::{get, put},
    Json, Router,
};
use nicgit_core::{conflicts::ConflictMode, roles::Role};
use serde::Deserialize;
use serde_json::{json, Value};

pub fn router() -> Router<AppState> {
    let base = "/api/projects/{owner}/{slug}";
    Router::new()
        .route(&format!("{base}/rules"), get(list))
        .route(&format!("{base}/rules/{{option}}"), put(set).delete(remove))
}

async fn list(State(state): State<AppState>, user: MaybeUser, Path((owner, slug)): Path<(String, String)>) -> ApiResult<Json<Value>> {
    let (p, _, git) = load(&state, &user, &owner, &slug).await?;
    let rules: Vec<OptionRule> = sqlx::query_as(
        "SELECT option, required_approvals, required_roles, allowed_modes, required_checks, protected_settings, dismiss_on_push \
         FROM option_rules WHERE project_id = $1 ORDER BY option",
    )
    .bind(p.id)
    .fetch_all(&state.db)
    .await?;
    // The settings keys of the default option's latest version, to pick protected ones from.
    let keys: Vec<String> = match git.resolve(&p.default_option).await {
        Ok(head) => json_file(&git, &head, "settings.json")
            .await?
            .and_then(|v| v.as_object().map(|o| o.keys().cloned().collect()))
            .unwrap_or_default(),
        Err(_) => vec![],
    };
    Ok(Json(json!({"rules": rules, "settings_keys": keys})))
}

#[derive(Deserialize)]
struct RuleBody {
    #[serde(default = "one")]
    required_approvals: i32,
    #[serde(default)]
    required_roles: Vec<String>,
    allowed_modes: Option<Vec<String>>,
    #[serde(default)]
    required_checks: Vec<String>,
    #[serde(default)]
    protected_settings: Vec<String>,
    #[serde(default = "yes")]
    dismiss_on_push: bool,
}

fn one() -> i32 {
    1
}
fn yes() -> bool {
    true
}

async fn set(
    State(state): State<AppState>,
    user: CurrentUser,
    Path((owner, slug, option)): Path<(String, String, String)>,
    Json(b): Json<RuleBody>,
) -> ApiResult<Json<Value>> {
    let (p, _) = access::load(&state, &owner, &slug, Some(user.id), Role::Lead).await?;
    if option.is_empty() || !crate::gitread::valid_rev(&option) {
        return Err(AppError::bad_request("not a design option name"));
    }
    if !(0..=20).contains(&b.required_approvals) {
        return Err(AppError::bad_request("approvals must be between 0 and 20"));
    }
    for r in &b.required_roles {
        r.parse::<Role>().map_err(|_| AppError::bad_request(format!("unknown role {r}")))?;
    }
    let modes = b.allowed_modes.unwrap_or_else(|| ConflictMode::ALL.iter().map(|m| m.as_str().to_string()).collect());
    for m in &modes {
        m.parse::<ConflictMode>().map_err(AppError::bad_request)?;
    }
    if modes.is_empty() {
        return Err(AppError::bad_request("allow at least one way to settle conflicts"));
    }
    sqlx::query(
        "INSERT INTO option_rules (project_id, option, required_approvals, required_roles, allowed_modes, required_checks, protected_settings, dismiss_on_push) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8) ON CONFLICT (project_id, option) DO UPDATE SET \
         required_approvals = $3, required_roles = $4, allowed_modes = $5, required_checks = $6, protected_settings = $7, dismiss_on_push = $8",
    )
    .bind(p.id)
    .bind(&option)
    .bind(b.required_approvals)
    .bind(&b.required_roles)
    .bind(&modes)
    .bind(&b.required_checks)
    .bind(&b.protected_settings)
    .bind(b.dismiss_on_push)
    .execute(&state.db)
    .await?;
    audit::record(&state.db, Some(p.owner_id), Some(user.id), "rule.set", &format!("{}:{option}", p.full_name()), json!({})).await?;
    Ok(Json(json!({"ok": true})))
}

async fn remove(State(state): State<AppState>, user: CurrentUser, Path((owner, slug, option)): Path<(String, String, String)>) -> ApiResult<Json<Value>> {
    let (p, _) = access::load(&state, &owner, &slug, Some(user.id), Role::Lead).await?;
    sqlx::query("DELETE FROM option_rules WHERE project_id = $1 AND option = $2").bind(p.id).bind(&option).execute(&state.db).await?;
    audit::record(&state.db, Some(p.owner_id), Some(user.id), "rule.remove", &format!("{}:{option}", p.full_name()), json!({})).await?;
    Ok(Json(json!({"ok": true})))
}
