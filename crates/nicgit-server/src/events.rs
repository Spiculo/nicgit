// SPDX-License-Identifier: AGPL-3.0-or-later
//! What happens after a push: activity, proposals following their options, background jobs.
use crate::{access::Project, app::AppState, error::ApiResult, git::RefUpdate};
use serde_json::json;
use uuid::Uuid;

pub async fn on_push(state: &AppState, project: &Project, user: Option<Uuid>, updates: &[RefUpdate]) -> ApiResult<()> {
    crate::activity::record(&state.db, project.id, user, "push", json!({"updates": updates})).await?;
    for u in updates {
        let (Some(option), Some(new)) = (u.option(), &u.new) else { continue };
        crate::proposals::on_option_moved(state, project, option, new).await?;
        crate::jobs::enqueue_summary(state, project, new).await?;
        crate::actions::on_head(state, project, option, new).await?;
    }
    crate::metering::sample_storage(state, project).await?;
    Ok(())
}

/// A worker finished a job: imports become a project's history (default option, activity, summaries).
pub async fn on_job_finished(
    state: &AppState,
    project: Uuid,
    job: Uuid,
    kind: &str,
    args: &serde_json::Value,
    result: Option<&serde_json::Value>,
    error: Option<&str>,
) -> ApiResult<()> {
    let (Some(result), None) = (result, error) else { return Ok(()) };
    if !matches!(kind, "import_nic" | "import_ifc" | "checks") {
        return Ok(());
    }
    let p: Project = sqlx::query_as(&format!(
        "SELECT {} FROM projects p JOIN accounts a ON a.id = p.owner_id WHERE p.id = $1",
        crate::access::PROJECT_COLUMNS
    ))
    .bind(project)
    .fetch_one(&state.db)
    .await?;
    if kind == "checks" {
        return crate::actions::on_finished(state, &p, job, args, result).await;
    }
    let actor = args["by"].as_str().and_then(|s| Uuid::parse_str(s).ok());
    // What an import did comes from the repository, not from the job's word.
    let refs = crate::git::refs(&state.cfg.git, &crate::repos::git_dir(&state.cfg, project)).await.map_err(crate::error::AppError::internal)?;
    let exists = |option: &str| refs.contains_key(&format!("refs/heads/{option}"));
    let heads: Vec<String> = match kind {
        "import_nic" => {
            if let Some(default) = result["default"].as_str().filter(|d| exists(d)) {
                crate::repos::set_head(&state.cfg, project, default).await.map_err(crate::error::AppError::internal)?;
                sqlx::query("UPDATE projects SET default_option = $2 WHERE id = $1").bind(project).bind(default).execute(&state.db).await?;
            }
            refs.iter().filter(|(name, _)| name.starts_with("refs/heads/")).map(|(_, id)| id.clone()).collect()
        }
        _ => result["commit"].as_str().filter(|c| refs.values().any(|id| id == c)).map(|c| vec![c.to_string()]).unwrap_or_default(),
    };
    crate::activity::record(&state.db, project, actor, "project.imported", json!({"kind": kind, "options": result["options"]})).await?;
    for h in heads {
        crate::jobs::enqueue_summary(state, &p, &h).await?;
    }
    crate::metering::sample_storage(state, &p).await?;
    Ok(())
}
