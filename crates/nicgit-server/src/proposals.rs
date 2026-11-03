// SPDX-License-Identifier: AGPL-3.0-or-later
//! Proposal logic shared by the API and push handling.
use crate::{
    access::{self, Project},
    app::AppState,
    error::ApiResult,
    gitread::Git,
    rules::{self, OptionRule},
};
use chrono::{DateTime, Utc};
use nicgit_core::approvals::{self, Evaluation, Requirement, Review, Verdict};
use nicgit_core::roles::Role;
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Proposal {
    pub id: Uuid,
    pub number: i32,
    pub title: String,
    pub body: String,
    pub source: String,
    pub target: String,
    pub author_id: Uuid,
    pub author: String,
    pub state: String,
    pub source_head: String,
    pub target_head: String,
    pub merge_commit: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub const COLUMNS: &str = "p.id, p.number, p.title, p.body, p.source, p.target, p.author_id, a.slug AS author, p.state, \
     p.source_head, p.target_head, p.merge_commit, p.created_at, p.updated_at";

pub async fn get(state: &AppState, project: Uuid, number: i32) -> ApiResult<Option<Proposal>> {
    Ok(sqlx::query_as(&format!(
        "SELECT {COLUMNS} FROM proposals p JOIN accounts a ON a.id = p.author_id WHERE p.project_id = $1 AND p.number = $2"
    ))
    .bind(project)
    .bind(number)
    .fetch_optional(&state.db)
    .await?)
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ReviewRow {
    pub user_id: Uuid,
    pub user: String,
    pub verdict: String,
    pub body: String,
    pub source_head: String,
    pub dismissed: bool,
    pub created_at: DateTime<Utc>,
}

pub async fn reviews(state: &AppState, proposal: Uuid) -> ApiResult<Vec<ReviewRow>> {
    Ok(sqlx::query_as(
        "SELECT r.user_id, a.slug AS user, r.verdict, r.body, r.source_head, r.dismissed, r.created_at \
         FROM reviews r JOIN accounts a ON a.id = r.user_id WHERE r.proposal_id = $1 ORDER BY r.created_at",
    )
    .bind(proposal)
    .fetch_all(&state.db)
    .await?)
}

/// The rule of the target option; an unprotected option needs nothing.
pub async fn rule(state: &AppState, project: Uuid, target: &str) -> ApiResult<(OptionRule, bool)> {
    Ok(match rules::rule_for(&state.db, project, target).await? {
        Some(r) => (r, true),
        None => (OptionRule::open(target), false),
    })
}

/// Top-level project settings keys the source option changed since it split from the target.
pub async fn settings_changed(git: &Git, target_head: &str, source_head: &str) -> ApiResult<Vec<String>> {
    let base = git.merge_base(target_head, source_head).await?;
    let read = |c: Option<String>| async move {
        match c {
            Some(c) => Ok::<_, crate::error::AppError>(crate::api::versions::json_file(git, &c, "settings.json").await?.unwrap_or(Value::Null)),
            None => Ok(Value::Null),
        }
    };
    let (a, b) = (read(base).await?, read(Some(source_head.to_string())).await?);
    let empty = serde_json::Map::new();
    let (a, b) = (a.as_object().unwrap_or(&empty), b.as_object().unwrap_or(&empty));
    let mut keys: Vec<String> = a.keys().chain(b.keys()).filter(|k| a.get(*k) != b.get(*k)).cloned().collect();
    keys.sort();
    keys.dedup();
    Ok(keys)
}

/// Approvals against the target option's rule: each reviewer's latest, undismissed verdict counts,
/// with their current role; the author's own approval never counts.
pub async fn evaluate(state: &AppState, project: &Project, p: &Proposal, git: &Git) -> ApiResult<(Evaluation, OptionRule, bool, Vec<String>)> {
    let (rule, protected) = rule(state, project.id, &p.target).await?;
    let mut latest: Vec<ReviewRow> = Vec::new();
    for r in reviews(state, p.id).await?.into_iter().rev() {
        if !r.dismissed && r.user_id != p.author_id && !latest.iter().any(|x| x.user_id == r.user_id) {
            latest.push(r);
        }
    }
    let mut counted = Vec::new();
    for r in &latest {
        let role = access::project_role(state, Some(r.user_id), project).await?.unwrap_or(Role::Viewer);
        if let Some(verdict) = Verdict::parse(&r.verdict) {
            counted.push(Review { user: r.user.clone(), role, verdict });
        }
    }
    let changed = if rule.protected_settings.is_empty() {
        vec![]
    } else {
        settings_changed(git, &p.target_head, &p.source_head).await?
    };
    let protected_changed: Vec<String> = changed.iter().filter(|k| rule.protected_settings.contains(k)).cloned().collect();
    let req = Requirement {
        approvals: rule.required_approvals.max(0) as u32,
        roles: rule.required_roles.iter().filter_map(|r| r.parse().ok()).collect(),
        protected_settings_changed: protected_changed,
    };
    Ok((approvals::evaluate(&req, &counted), rule, protected, changed))
}

pub async fn set_ref(state: &AppState, project: &Project, number: i32, commit: &str) -> ApiResult<()> {
    crate::git::run(&state.cfg.git, &project.repo_dir(state).join(".git"), &["update-ref", &format!("refs/proposals/{number}/head"), commit])
        .await
        .map_err(crate::error::AppError::internal)?;
    Ok(())
}

/// A design option got new versions: open proposals from it follow its head (and approvals given to an
/// earlier head are dismissed when the target's rule says so); proposals into it note the new target.
pub async fn on_option_moved(state: &AppState, project: &Project, option: &str, head: &str) -> ApiResult<()> {
    // Moving the head and dismissing earlier approvals happen together: a reader never sees a proposal
    // approved on versions nobody reviewed.
    let mut tx = state.db.begin().await?;
    let moved: Vec<(Uuid, i32, String)> = sqlx::query_as(
        "UPDATE proposals SET source_head = $3, updated_at = now()          WHERE project_id = $1 AND source = $2 AND state = 'open' AND source_head <> $3 RETURNING id, number, target",
    )
    .bind(project.id)
    .bind(option)
    .bind(head)
    .fetch_all(&mut *tx)
    .await?;
    for (id, _, target) in &moved {
        let dismiss = rules::rule_for(&state.db, project.id, target).await?.is_none_or(|r| r.dismiss_on_push);
        if dismiss {
            sqlx::query("UPDATE reviews SET dismissed = true WHERE proposal_id = $1 AND source_head <> $2 AND NOT dismissed")
                .bind(id)
                .bind(head)
                .execute(&mut *tx)
                .await?;
        }
    }
    sqlx::query("UPDATE proposals SET target_head = $3, updated_at = now() WHERE project_id = $1 AND target = $2 AND state = 'open'")
        .bind(project.id)
        .bind(option)
        .bind(head)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    for (_, number, _) in moved {
        set_ref(state, project, number, head).await?;
    }
    Ok(())
}
