// SPDX-License-Identifier: AGPL-3.0-or-later
//! AI through the owner's cloud AI router (spec section 13, docs/ai-router.md).
//!
//! Rules: off by default per account; nothing is sent unless the project's owner account turned AI on
//! and a router is configured; only the engine's structured data is sent (never raw records or files);
//! every answer is metered (`ai_units`, as the router reports) and stopped by a hard budget.
use crate::{
    access::Project,
    app::AppState,
    error::{ApiResult, AppError},
    proposals::Proposal,
};
use axum::http::StatusCode;
use serde_json::{json, Value};

pub async fn enabled(state: &AppState, account: uuid::Uuid) -> ApiResult<bool> {
    Ok(sqlx::query_scalar("SELECT enabled FROM ai_settings WHERE account_id = $1").bind(account).fetch_optional(&state.db).await?.unwrap_or(false))
}

/// Refuse early when AI cannot be used for this project's owner.
pub async fn ensure_available(state: &AppState, project: &Project) -> ApiResult<()> {
    if !enabled(state, project.owner_id).await? {
        return Err(AppError::conflict("ai_disabled", "AI is turned off for this account"));
    }
    if state.cfg.ai_router_url.is_none() {
        return Err(AppError::conflict("ai_unavailable", "no AI router is configured on this server"));
    }
    crate::metering::check(state, project.owner_id, "ai_units").await
}

pub struct Answer {
    pub text: String,
    pub units: i64,
}

/// One chat request to the router: `POST {router}/v1/chat`.
pub async fn chat(state: &AppState, project: &Project, purpose: &str, system: &str, user: &str, lang: &str) -> ApiResult<Answer> {
    let url = state.cfg.ai_router_url.as_deref().ok_or_else(|| AppError::conflict("ai_unavailable", "no AI router is configured"))?;
    let body = json!({
        "model": "auto",
        "messages": [{"role": "system", "content": system}, {"role": "user", "content": user}],
        "metadata": {"account": project.owner_id, "project": project.id, "purpose": purpose, "language": lang},
    });
    let mut req = reqwest::Client::new().post(format!("{}/v1/chat", url.trim_end_matches('/'))).json(&body).timeout(std::time::Duration::from_secs(120));
    if let Some(key) = &state.cfg.ai_router_key {
        req = req.bearer_auth(key);
    }
    let r = req.send().await.map_err(|e| AppError::new(StatusCode::BAD_GATEWAY, "ai_router", format!("the AI router did not answer: {e}")))?;
    if !r.status().is_success() {
        return Err(AppError::new(StatusCode::BAD_GATEWAY, "ai_router", format!("the AI router answered {}", r.status())));
    }
    let v: Value = r.json().await.map_err(|e| AppError::new(StatusCode::BAD_GATEWAY, "ai_router", e.to_string()))?;
    let text = v["text"].as_str().unwrap_or_default().trim().to_string();
    let units = v["usage"]["units"].as_i64().unwrap_or(1).max(0);
    crate::metering::record(state, project.owner_id, Some(project.id), "ai_units", units).await?;
    Ok(Answer { text, units })
}

fn subject(p: &Proposal) -> String {
    format!("proposal:{}:{}:{}", p.number, p.target_head, p.source_head)
}

/// The cached summary of this proposal at its current heads, if one was made.
pub async fn proposal_summary_cached(state: &AppState, project: &Project, p: &Proposal) -> ApiResult<Option<Value>> {
    let row: Option<(String, chrono::DateTime<chrono::Utc>)> = sqlx::query_as("SELECT text, created_at FROM ai_summaries WHERE project_id = $1 AND subject = $2")
        .bind(project.id)
        .bind(subject(p))
        .fetch_optional(&state.db)
        .await?;
    Ok(row.map(|(text, at)| json!({"text": text, "created_at": at, "ai": true})))
}

/// The structured diff as compact lines, so the model reads the engine's facts, not raw records.
pub fn describe_changes(compare: &Value, limit: usize) -> String {
    let mut lines = Vec::new();
    for d in compare["details"].as_array().into_iter().flatten().take(limit) {
        let mut line = format!(
            "- {} {} \"{}\" on {}",
            d["kind"].as_str().unwrap_or(""),
            d["type"].as_str().unwrap_or(""),
            d["name"].as_str().unwrap_or(""),
            d["level"].as_str().filter(|l| !l.is_empty()).unwrap_or("no level")
        );
        let fields: Vec<String> = d["attributes"]
            .as_array()
            .into_iter()
            .flatten()
            .chain(d["properties"].as_array().into_iter().flatten())
            .map(|f| format!("{}: {} -> {}", f["name"].as_str().unwrap_or(""), f["old"], f["new"]))
            .collect();
        if !fields.is_empty() {
            line.push_str(&format!(" ({})", fields.join("; ")));
        }
        lines.push(line);
    }
    let total = compare["details"].as_array().map_or(0, Vec::len);
    if total > limit {
        lines.push(format!("- … and {} more changed elements", total - limit));
    }
    if compare["settings"].as_array().is_some_and(|s| !s.is_empty()) {
        lines.push("- the project settings changed".into());
    }
    lines.join("\n")
}

const SYSTEM: &str = "You review design changes in a building information model for architects. Using only the change \
list given, write a short summary (at most 6 sentences) of what the proposal changes and anything a reviewer should \
check, grouped by level where it helps. Use the architects' words (wall, door, level), not IFC class names. Do not \
invent changes that are not listed.";

pub async fn summarize_proposal(state: &AppState, project: &Project, p: &Proposal, compare: &Value, lang: &str) -> ApiResult<Value> {
    let language = if lang == "es" { "Spanish" } else { "English" };
    let user = format!(
        "Proposal: {}\n{}\n\nChanges from {} into {}:\n{}\n\nAnswer in {language}.",
        p.title,
        p.body,
        crate::api::versions::option_label(&p.source),
        crate::api::versions::option_label(&p.target),
        describe_changes(compare, 200)
    );
    let answer = chat(state, project, "proposal_summary", SYSTEM, &user, lang).await?;
    sqlx::query(
        "INSERT INTO ai_summaries (project_id, subject, text, units) VALUES ($1, $2, $3, $4) \
         ON CONFLICT (project_id, subject) DO UPDATE SET text = $3, units = $4, created_at = now()",
    )
    .bind(project.id)
    .bind(subject(p))
    .bind(&answer.text)
    .bind(answer.units)
    .execute(&state.db)
    .await?;
    Ok(json!({"text": answer.text, "ai": true, "units": answer.units}))
}
