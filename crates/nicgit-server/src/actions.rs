// SPDX-License-Identifier: AGPL-3.0-or-later
//! Actions: the checks a project asks for (schema, IDS, clash, model), run by engine workers on a version.
//! They are the billable compute (decision N7): metered in `action_seconds` and stopped by a hard budget.
use crate::{
    access::Project,
    app::AppState,
    error::{ApiResult, AppError},
    jobs::{self, JobState},
};
use serde_json::{json, Value};
use uuid::Uuid;

pub const CHECKS: [&str; 4] = ["schema", "ids", "clash", "model"];

pub fn validate(names: &[String]) -> ApiResult<()> {
    match names.iter().find(|n| !CHECKS.contains(&n.as_str())) {
        Some(n) => Err(AppError::bad_request(format!("unknown check {n}; checks are schema, ids, clash and model"))),
        None => Ok(()),
    }
}

fn key(project: &Project, commit: &str, names: &[String]) -> String {
    let mut sorted = names.to_vec();
    sorted.sort();
    jobs::key(project, "checks", &[commit, &sorted.join(",")])
}

/// The checks job for this version, queued if it is not there yet. Refused when the budget is spent.
pub async fn run(state: &AppState, project: &Project, commit: &str, names: &[String]) -> ApiResult<JobState> {
    validate(names)?;
    if let Some(job) = existing(state, project, commit, names).await? {
        if !job.failed() {
            return Ok(job);
        }
    }
    crate::metering::check(state, project.owner_id, "action_seconds").await?;
    jobs::enqueue(&state.db, project.id, "checks", json!({"rev": commit, "checks": names}), Some(&key(project, commit, names))).await
}

pub async fn existing(state: &AppState, project: &Project, commit: &str, names: &[String]) -> ApiResult<Option<JobState>> {
    Ok(sqlx::query_as("SELECT id, kind, status, progress, result, error FROM jobs WHERE cache_key = $1")
        .bind(key(project, commit, names))
        .fetch_optional(&state.db)
        .await?)
}

/// Queue the option's required checks for a new head; a spent budget skips them (recorded as activity).
pub async fn on_head(state: &AppState, project: &Project, option: &str, commit: &str) -> ApiResult<()> {
    let Some(rule) = crate::rules::rule_for(&state.db, project.id, option).await? else { return Ok(()) };
    if rule.required_checks.is_empty() {
        return Ok(());
    }
    match run(state, project, commit, &rule.required_checks).await {
        Ok(_) => Ok(()),
        Err(e) if e.code == "budget_exceeded" => {
            crate::activity::record(&state.db, project.id, None, "actions.skipped", json!({"reason": "budget", "commit": commit})).await?;
            Ok(())
        }
        Err(e) => Err(e),
    }
}

/// The state of the required checks on a commit: none, pending, passed or failed.
pub async fn status(state: &AppState, project: &Project, commit: &str, required: &[String]) -> ApiResult<Value> {
    if required.is_empty() {
        return Ok(json!({"required": [], "state": "none"}));
    }
    let job = existing(state, project, commit, required).await?;
    let st = match &job {
        None => "pending",
        Some(j) if j.done() => {
            if j.result.as_ref().is_some_and(|r| r["passed"] == true) {
                "passed"
            } else {
                "failed"
            }
        }
        Some(j) if j.failed() => "failed",
        Some(_) => "pending",
    };
    Ok(json!({"required": required, "state": st, "job": job.as_ref().map(|j| j.id), "result": job.and_then(|j| j.result)}))
}

/// Meter a finished checks run: whole seconds, rounded up, at least one.
/// The time billed and the version named come from the server's own records, not from the job: a model that
/// took over the engine could say anything about itself (decision N36).
pub async fn on_finished(state: &AppState, project: &Project, job: Uuid, args: &Value, result: &Value) -> ApiResult<()> {
    let seconds: f64 = sqlx::query_scalar("SELECT coalesce(extract(epoch FROM finished_at - started_at)::float8, 0) FROM jobs WHERE id = $1").bind(job).fetch_one(&state.db).await?;
    crate::metering::record(state, project.owner_id, Some(project.id), "action_seconds", seconds.ceil().max(1.0) as i64).await?;
    let passed = result["passed"].as_bool().unwrap_or(false);
    crate::activity::record(&state.db, project.id, None, "checks.finished", json!({"commit": args["rev"], "passed": passed})).await?;
    Ok(())
}
