// SPDX-License-Identifier: AGPL-3.0-or-later
//! Usage metering and budgets (spec section 12).
//!
//! Kinds: `storage_bytes` and `package_storage_bytes` are sampled gauges (the latest sample per project
//! counts); everything else (`action_seconds`, `egress_bytes`, `package_egress_bytes`, `ai_units`) is summed
//! over the current calendar month. A hard limit blocks only Actions and AI, never reading or pushing.
use crate::{
    access::Project,
    app::AppState,
    error::{ApiResult, AppError},
};
use axum::http::StatusCode;
use serde_json::{json, Value};
use std::path::Path;
use uuid::Uuid;

pub const GAUGES: [&str; 2] = ["storage_bytes", "package_storage_bytes"];
pub const KINDS: [&str; 6] = ["storage_bytes", "egress_bytes", "action_seconds", "ai_units", "package_storage_bytes", "package_egress_bytes"];

pub async fn record(state: &AppState, account: Uuid, project: Option<Uuid>, kind: &str, quantity: i64) -> ApiResult<()> {
    sqlx::query("INSERT INTO usage_events (account_id, project_id, kind, quantity) VALUES ($1, $2, $3, $4)")
        .bind(account)
        .bind(project)
        .bind(kind)
        .bind(quantity)
        .execute(&state.db)
        .await?;
    Ok(())
}

/// This month's usage of an account, by kind.
pub async fn usage(state: &AppState, account: Uuid) -> ApiResult<Vec<(String, i64)>> {
    let rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT kind, sum(quantity)::bigint FROM usage_events \
         WHERE account_id = $1 AND kind <> ALL($2) AND created_at >= date_trunc('month', now()) GROUP BY kind \
         UNION ALL \
         SELECT kind, sum(q)::bigint FROM ( \
           SELECT DISTINCT ON (kind, project_id) kind, quantity AS q FROM usage_events \
           WHERE account_id = $1 AND kind = ANY($2) ORDER BY kind, project_id, created_at DESC, id DESC) g GROUP BY kind",
    )
    .bind(account)
    .bind(&GAUGES[..])
    .fetch_all(&state.db)
    .await?;
    Ok(rows)
}

pub async fn budgets(state: &AppState, account: Uuid) -> ApiResult<Vec<Value>> {
    let rows: Vec<(String, Option<i64>, Option<i64>)> =
        sqlx::query_as("SELECT kind, soft_limit, hard_limit FROM budgets WHERE account_id = $1 ORDER BY kind").bind(account).fetch_all(&state.db).await?;
    Ok(rows.into_iter().map(|(k, s, h)| json!({"kind": k, "soft_limit": s, "hard_limit": h})).collect())
}

/// Refuse (402) when the account has used up its hard limit for this kind.
pub async fn check(state: &AppState, account: Uuid, kind: &str) -> ApiResult<()> {
    let hard: Option<i64> =
        sqlx::query_scalar("SELECT hard_limit FROM budgets WHERE account_id = $1 AND kind = $2").bind(account).bind(kind).fetch_optional(&state.db).await?.flatten();
    let Some(hard) = hard else { return Ok(()) };
    let used = usage(state, account).await?.into_iter().find(|(k, _)| k == kind).map_or(0, |(_, q)| q);
    if used >= hard {
        return Err(AppError::new(
            StatusCode::PAYMENT_REQUIRED,
            "budget_exceeded",
            format!("this account has used its {kind} budget for the month ({used} of {hard})"),
        ));
    }
    Ok(())
}

fn dir_size(p: &Path) -> u64 {
    let Ok(rd) = std::fs::read_dir(p) else { return 0 };
    rd.filter_map(Result::ok)
        .map(|e| match e.file_type() {
            Ok(t) if t.is_dir() => dir_size(&e.path()),
            Ok(_) => e.metadata().map(|m| m.len()).unwrap_or(0),
            Err(_) => 0,
        })
        .sum()
}

/// Record the repository's size on disk (sampled after each push).
pub async fn sample_storage(state: &AppState, project: &Project) -> ApiResult<()> {
    let dir = project.repo_dir(state);
    let size = tokio::task::spawn_blocking(move || dir_size(&dir)).await.map_err(AppError::internal)?;
    record(state, project.owner_id, Some(project.id), "storage_bytes", size as i64).await
}
