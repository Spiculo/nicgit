// SPDX-License-Identifier: AGPL-3.0-or-later
//! CI / Actions: the checks jobs a project already ran, read back for the web app.
//!
//! This is a view over the existing `jobs` queue, not a second execution model: a run is a `checks`
//! job with its commit, checks, result, timing and report. The Actions runner still does the work.
use crate::{
    access::{self, Project},
    actions,
    app::AppState,
    auth::MaybeUser,
    error::{ApiResult, AppError},
    gitread::Git,
    rules,
};
use axum::{
    extract::{Path, Query, State},
    response::Html,
    routing::get,
    Json, Router,
};
use chrono::{DateTime, Utc};
use nicgit_core::roles::Role;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    let base = "/api/projects/{owner}/{slug}";
    Router::new()
        .route(&format!("{base}/ci/runs"), get(runs))
        .route(&format!("{base}/ci/runs/{{id}}"), get(run))
        .route(&format!("{base}/ci/checks"), get(checks))
        .route(&format!("{base}/ci/runs/{{id}}/report.html"), get(report))
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
struct RunRow {
    id: Uuid,
    status: String,
    progress: f32,
    result: Option<Value>,
    error: Option<String>,
    args: Value,
    created_at: DateTime<Utc>,
    started_at: Option<DateTime<Utc>>,
    finished_at: Option<DateTime<Utc>>,
}

const COLUMNS: &str =
    "id, status, progress, result, error, args, created_at, started_at, finished_at";

fn run_json(run: &RunRow, full: bool) -> Value {
    let commit = run.args.get("rev").and_then(Value::as_str).unwrap_or("");
    let checks = run.args.get("checks").cloned().unwrap_or_else(|| json!([]));
    let summary =
        run.result
            .as_ref()
            .and_then(|r| r.get("checks"))
            .and_then(Value::as_array)
            .map(|items| {
                items.iter().map(|c| json!({
            "check": c.get("check"), "title": c.get("title"), "failed": c.get("failed"),
            "total": c.get("total"), "counts": c.get("counts"), "error": c.get("error"),
        })).collect::<Vec<_>>()
            })
            .unwrap_or_default();
    let duration = match (run.started_at, run.finished_at) {
        (Some(start), Some(end)) => Some((end - start).num_milliseconds() as f64 / 1000.0),
        _ => None,
    };
    json!({
        "id": run.id,
        "commit": commit,
        "checks": checks,
        "status": run.status,
        "progress": run.progress,
        "passed": run.result.as_ref().and_then(|r| r.get("passed")).and_then(Value::as_bool),
        "seconds": run.result.as_ref().and_then(|r| r.get("seconds")).cloned(),
        "result": if full { run.result.clone() } else { None },
        "summary": summary,
        "error": run.error,
        "created_at": run.created_at,
        "started_at": run.started_at,
        "finished_at": run.finished_at,
        "duration": duration,
    })
}

#[derive(Deserialize)]
struct RunsQuery {
    limit: Option<i64>,
    option: Option<String>,
    rev: Option<String>,
}

/// The checks runs of a project, newest first. `option` narrows to that option's current version;
/// `rev` narrows to one commit. Both are resolved through the engine's own repository reader.
async fn runs(
    State(state): State<AppState>,
    MaybeUser(user): MaybeUser,
    Path((owner, slug)): Path<(String, String)>,
    Query(q): Query<RunsQuery>,
) -> ApiResult<Json<Value>> {
    let (p, _) = access::load(&state, &owner, &slug, user.map(|u| u.id), Role::Viewer).await?;
    let git = Git::new(&p.repo_dir(&state));
    let mut filter = q.rev.filter(|r| !r.trim().is_empty());
    if let Some(option) = q.option.filter(|o| !o.trim().is_empty()) {
        filter = Some(git.resolve(&option).await?);
    }
    let limit = q.limit.unwrap_or(50).clamp(1, 200);
    let rows: Vec<RunRow> = sqlx::query_as(&format!(
        "SELECT {COLUMNS} FROM jobs WHERE project_id = $1 AND kind = 'checks' \
         AND ($2::text IS NULL OR args->>'rev' = $2) ORDER BY created_at DESC LIMIT $3"
    ))
    .bind(p.id)
    .bind(filter)
    .bind(limit)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(
        json!({"runs": rows.iter().map(|row| run_json(row, false)).collect::<Vec<_>>()}),
    ))
}

/// One run with its full result (the page expands it in place; the report link is the same data as HTML).
async fn run(
    State(state): State<AppState>,
    MaybeUser(user): MaybeUser,
    Path((owner, slug, id)): Path<(String, String, Uuid)>,
) -> ApiResult<Json<Value>> {
    let (p, _) = access::load(&state, &owner, &slug, user.map(|u| u.id), Role::Viewer).await?;
    let row: Option<RunRow> = sqlx::query_as(&format!(
        "SELECT {COLUMNS} FROM jobs WHERE id = $1 AND project_id = $2 AND kind = 'checks'"
    ))
    .bind(id)
    .bind(p.id)
    .fetch_optional(&state.db)
    .await?;
    let row = row.ok_or_else(AppError::not_found)?;
    Ok(Json(json!({"run": run_json(&row, true)})))
}

/// The required checks of every option and their latest result. An unprotected option has none.
async fn checks(
    State(state): State<AppState>,
    MaybeUser(user): MaybeUser,
    Path((owner, slug)): Path<(String, String)>,
) -> ApiResult<Json<Value>> {
    let (p, _) = access::load(&state, &owner, &slug, user.map(|u| u.id), Role::Viewer).await?;
    let git = Git::new(&p.repo_dir(&state));
    let options = git.options().await?;
    let mut out = Vec::new();
    for option in options {
        let stored = rules::rule_for(&state.db, p.id, &option).await?;
        let protected = stored.is_some();
        let rule = stored.unwrap_or_else(|| rules::OptionRule::open(&option));
        let head = git.resolve(&option).await.unwrap_or_default();
        let status = if head.is_empty() || rule.required_checks.is_empty() {
            json!({"required": rule.required_checks, "state": "none"})
        } else {
            actions::status(&state, &p, &head, &rule.required_checks).await?
        };
        out.push(json!({
            "option": option,
            "head": head,
            "protected": protected,
            "required": rule.required_checks,
            "approvals": rule.required_approvals,
            "state": status.get("state").cloned().unwrap_or_else(|| json!("none")),
            "job": status.get("job").cloned().unwrap_or(Value::Null),
            "result": status.get("result").cloned().unwrap_or(Value::Null),
        }));
    }
    Ok(Json(json!({"options": out, "checks": actions::CHECKS})))
}

/// A stored, printable report of one run. It is generated from the job's own result; no engine work.
async fn report(
    State(state): State<AppState>,
    MaybeUser(user): MaybeUser,
    Path((owner, slug, id)): Path<(String, String, Uuid)>,
) -> ApiResult<Html<String>> {
    let (p, _) = access::load(&state, &owner, &slug, user.map(|u| u.id), Role::Viewer).await?;
    let row: Option<RunRow> = sqlx::query_as(&format!(
        "SELECT {COLUMNS} FROM jobs WHERE id = $1 AND project_id = $2 AND kind = 'checks'"
    ))
    .bind(id)
    .bind(p.id)
    .fetch_optional(&state.db)
    .await?;
    let row = row.ok_or_else(AppError::not_found)?;
    Ok(Html(render_report(&p, &row)))
}

fn esc(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn render_report(project: &Project, run: &RunRow) -> String {
    let commit = run.args.get("rev").and_then(Value::as_str).unwrap_or("");
    let mut checks = String::new();
    for check in run
        .result
        .as_ref()
        .and_then(|r| r.get("checks"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let title = esc(check
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("Check"));
        let failed = check
            .get("failed")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let counts = check.get("counts").cloned().unwrap_or_else(|| json!({}));
        let error = counts.get("error").and_then(Value::as_i64).unwrap_or(0);
        let warning = counts.get("warning").and_then(Value::as_i64).unwrap_or(0);
        let total = check.get("total").and_then(Value::as_i64).unwrap_or(0);
        checks.push_str(&format!(
            "<section><h2>{} <span class=\"{}\">{}</span></h2><p>{} issue(s): {} error(s), {} warning(s)</p>",
            title,
            if failed { "bad" } else { "ok" },
            if failed { "Failed" } else { "Passed" },
            total,
            error,
            warning
        ));
        if let Some(message) = check.get("error").and_then(Value::as_str) {
            checks.push_str(&format!("<p class=\"bad\">{}</p>", esc(message)));
        }
        if let Some(issues) = check.get("issues").and_then(Value::as_array) {
            checks.push_str("<table><thead><tr><th>Severity</th><th>Message</th><th>Elements</th></tr></thead><tbody>");
            for issue in issues {
                let severity = esc(issue.get("severity").and_then(Value::as_str).unwrap_or(""));
                let message = esc(issue.get("message").and_then(Value::as_str).unwrap_or(""));
                let elements = issue
                    .get("guids")
                    .and_then(Value::as_array)
                    .map(|g| {
                        g.iter()
                            .filter_map(Value::as_str)
                            .take(8)
                            .map(esc)
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_default();
                checks.push_str(&format!(
                    "<tr><td>{severity}</td><td>{message}</td><td>{elements}</td></tr>"
                ));
            }
            checks.push_str("</tbody></table>");
        }
        checks.push_str("</section>");
    }
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>Checks — {project}</title>\
         <style>body{{font:15px/1.45 system-ui,sans-serif;max-width:60rem;margin:2rem auto;padding:0 1rem;color:#1d2328}}\
         h1{{font-size:1.5rem}}h2{{font-size:1.1rem;margin-top:1.5rem}}.ok{{color:#1d7a3a}}.bad{{color:#a12a2a}}\
         table{{border-collapse:collapse;width:100%;margin:.5rem 0}}th,td{{border-bottom:1px solid #d8dde1;padding:.35rem .5rem;text-align:left;vertical-align:top}}\
         code{{background:#f1f3f4;padding:.1rem .25rem}}</style></head><body>\
         <h1>Checks for {project}</h1><p>Version <code>{commit}</code> · status <strong>{status}</strong></p>\
         {checks}</body></html>",
        project = esc(&project.full_name()),
        commit = esc(commit),
        status = esc(&run.status),
        checks = if checks.is_empty() { "<p>No check result is stored for this run.</p>".to_string() } else { checks },
    )
}
