// SPDX-License-Identifier: AGPL-3.0-or-later
//! Usage metering and budgets (spec section 12).
use crate::{access::Project, app::AppState, error::ApiResult};
use std::path::Path;
use uuid::Uuid;

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
    let size = tokio::task::spawn_blocking(move || dir_size(&dir)).await.map_err(crate::error::AppError::internal)?;
    record(state, project.owner_id, Some(project.id), "storage_bytes", size as i64).await
}
