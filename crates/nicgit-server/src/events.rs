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
    }
    crate::metering::sample_storage(state, project).await?;
    Ok(())
}

/// A worker finished a job: follow-ups such as Actions metering and AI summaries.
pub async fn on_job_finished(
    state: &AppState,
    project: Uuid,
    _job: Uuid,
    kind: &str,
    _args: &serde_json::Value,
    _result: Option<&serde_json::Value>,
    _error: Option<&str>,
) -> ApiResult<()> {
    let _ = (state, project, kind);
    Ok(())
}
