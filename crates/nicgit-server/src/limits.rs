// SPDX-License-Identifier: AGPL-3.0-or-later
//! How much one person can store (decision N33). Anyone can sign up, so space is limited per file, per project
//! and per account. Sizes are measured on disk when asked: they cannot drift from what is really there, and a
//! limit is only ever checked just before something large arrives.
use crate::{
    app::AppState,
    config::Config,
    error::{ApiResult, AppError},
    metering::dir_size,
    repos,
};
use axum::http::StatusCode;
use uuid::Uuid;

pub fn bytes(mb: u64) -> u64 {
    mb.saturating_mul(1 << 20)
}

pub fn storage_full(message: impl Into<String>) -> AppError {
    AppError::new(StatusCode::PAYLOAD_TOO_LARGE, "storage_full", message.into())
}

fn limit_reached(message: String) -> AppError {
    AppError::new(StatusCode::FORBIDDEN, "limit_reached", message)
}

async fn measured<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> ApiResult<T> {
    tokio::task::spawn_blocking(work).await.map_err(AppError::internal)
}

/// Bytes an account uses: the repositories it owns and the uploads waiting to be imported into them.
pub async fn account_bytes(state: &AppState, account: Uuid) -> ApiResult<u64> {
    let projects: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM projects WHERE owner_id = $1").bind(account).fetch_all(&state.db).await?;
    let cfg = state.cfg.clone();
    measured(move || projects.iter().map(|p| dir_size(&repos::dir(&cfg, *p)) + dir_size(&repos::uploads_dir(&cfg, *p))).sum()).await
}

/// How much more one project can take, and if that is nothing, the reason to show.
pub struct Room {
    pub bytes: u64,
    pub full: Option<String>,
}

/// The room for a push. An import's own upload does not count against it: it goes away when the import ends.
pub async fn room(state: &AppState, project: Uuid, importing: bool) -> ApiResult<Room> {
    let owner: Uuid = sqlx::query_scalar("SELECT owner_id FROM projects WHERE id = $1").bind(project).fetch_one(&state.db).await?;
    let used = account_bytes(state, owner).await?;
    let cfg = state.cfg.clone();
    let (repo, upload) = measured(move || (dir_size(&repos::dir(&cfg, project)), dir_size(&repos::uploads_dir(&cfg, project)))).await?;
    let used = if importing { used.saturating_sub(upload) } else { used };
    let cfg = &state.cfg;
    let in_project = bytes(cfg.max_repo_mb).saturating_sub(repo);
    let in_account = bytes(cfg.max_account_mb).saturating_sub(used);
    let full = if in_project == 0 {
        Some(format!("this project has reached its {} MB limit", cfg.max_repo_mb))
    } else if in_account == 0 {
        Some(format!("this account has reached its {} MB limit", cfg.max_account_mb))
    } else {
        None
    };
    Ok(Room { bytes: in_project.min(in_account), full })
}

/// Refuse a file of this size when its account could not hold it.
pub async fn check_fits(state: &AppState, account: Uuid, size: u64) -> ApiResult<()> {
    let limit = bytes(state.cfg.max_account_mb);
    if account_bytes(state, account).await?.saturating_add(size) > limit {
        return Err(storage_full(format!("this account has reached its {} MB limit", state.cfg.max_account_mb)));
    }
    Ok(())
}

pub async fn check_new_project(state: &AppState, account: Uuid) -> ApiResult<()> {
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM projects WHERE owner_id = $1").bind(account).fetch_one(&state.db).await?;
    if count >= state.cfg.max_projects as i64 {
        return Err(limit_reached(format!("an account can have at most {} projects", state.cfg.max_projects)));
    }
    Ok(())
}

/// Each organisation has its own storage limit, so the number a person can create is limited too.
pub async fn check_new_org(state: &AppState, user: Uuid) -> ApiResult<()> {
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM org_members WHERE user_id = $1 AND role = 'admin'").bind(user).fetch_one(&state.db).await?;
    if count >= state.cfg.max_orgs as i64 {
        return Err(limit_reached(format!("you can administer at most {} organisations", state.cfg.max_orgs)));
    }
    Ok(())
}

/// What the account page shows.
pub async fn summary(state: &AppState, account: Uuid) -> ApiResult<serde_json::Value> {
    let cfg = &state.cfg;
    Ok(serde_json::json!({
        "used": account_bytes(state, account).await?,
        "limit": bytes(cfg.max_account_mb),
        "project_limit": bytes(cfg.max_repo_mb),
        "file_limit": bytes(cfg.max_file_mb),
    }))
}

/// Cached exports can be made again, so a project going over its allowance loses the oldest rather than being
/// refused anything. `keep` is the file just written, and files still being written are left alone.
pub fn trim_cache(cfg: &Config, project: Uuid, keep: &std::path::Path) {
    let mut files: Vec<(std::time::SystemTime, u64, std::path::PathBuf)> = Vec::new();
    let mut total = 0u64;
    for folder in ["ifc", "exports"] {
        let Ok(entries) = std::fs::read_dir(repos::cache_dir(cfg, project).join(folder)) else { continue };
        for e in entries.filter_map(Result::ok) {
            let Ok(meta) = e.metadata() else { continue };
            total += meta.len();
            let path = e.path();
            let writing = e.file_name().to_string_lossy().starts_with(".part-");
            if !writing && path != keep {
                files.push((meta.modified().unwrap_or(std::time::UNIX_EPOCH), meta.len(), path));
            }
        }
    }
    let allowed = bytes(cfg.max_repo_mb);
    files.sort();
    for (_, len, path) in files {
        if total <= allowed {
            break;
        }
        if std::fs::remove_file(&path).is_ok() {
            total = total.saturating_sub(len);
        }
    }
}
