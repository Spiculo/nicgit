// SPDX-License-Identifier: AGPL-3.0-or-later
//! Who may do what on a project. A project a caller cannot read does not exist for them (404).
use crate::{
    app::AppState,
    error::{ApiResult, AppError},
};
use chrono::{DateTime, Utc};
use nicgit_core::roles::Role;
use serde::Serialize;
use serde_json::{json, Value};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct Project {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub owner_slug: String,
    pub owner_kind: String,
    pub slug: String,
    pub description: String,
    pub visibility: String,
    pub default_option: String,
    pub created_at: DateTime<Utc>,
}

pub const PROJECT_COLUMNS: &str = "p.id, p.owner_id, a.slug AS owner_slug, a.kind AS owner_kind, p.slug, p.description, \
     p.visibility, p.default_option, p.created_at";

impl Project {
    pub fn full_name(&self) -> String {
        format!("{}/{}", self.owner_slug, self.slug)
    }

    pub fn is_public(&self) -> bool {
        self.visibility == "public"
    }

    pub fn repo_dir(&self, state: &AppState) -> PathBuf {
        crate::repos::dir(&state.cfg, self.id)
    }

    pub fn to_json(&self, role: Option<Role>) -> Value {
        json!({
            "id": self.id, "owner": self.owner_slug, "owner_kind": self.owner_kind, "slug": self.slug,
            "full_name": self.full_name(), "description": self.description, "visibility": self.visibility,
            "default_option": self.default_option, "created_at": self.created_at, "role": role,
        })
    }
}

pub async fn find(state: &AppState, owner: &str, slug: &str) -> ApiResult<Option<Project>> {
    Ok(sqlx::query_as::<_, Project>(&format!(
        "SELECT {PROJECT_COLUMNS} FROM projects p JOIN accounts a ON a.id = p.owner_id WHERE a.slug = $1 AND p.slug = $2"
    ))
    .bind(owner)
    .bind(slug)
    .fetch_optional(&state.db)
    .await?)
}

/// The caller's highest role on the project, from ownership, organisation membership, a direct grant
/// or a team grant; public projects give everyone at least `Viewer`. Site admins get nothing implicitly:
/// customer data is private even from the operator's accounts.
pub async fn project_role(state: &AppState, user: Option<Uuid>, project: &Project) -> ApiResult<Option<Role>> {
    let mut best: Option<Role> = project.is_public().then_some(Role::Viewer);
    if let Some(u) = user {
        if project.owner_kind == "user" && project.owner_id == u {
            return Ok(Some(Role::Admin));
        }
        let roles: Vec<String> = sqlx::query_scalar(
            "SELECT role FROM org_members WHERE org_id = $1 AND user_id = $3 \
             UNION ALL SELECT role FROM project_members WHERE project_id = $2 AND user_id = $3 \
             UNION ALL SELECT tpa.role FROM team_project_access tpa JOIN team_members tm ON tm.team_id = tpa.team_id \
                       WHERE tpa.project_id = $2 AND tm.user_id = $3",
        )
        .bind(project.owner_id)
        .bind(project.id)
        .bind(u)
        .fetch_all(&state.db)
        .await?;
        for r in roles.iter().filter_map(|r| r.parse::<Role>().ok()) {
            best = best.max(Some(r));
        }
    }
    Ok(best)
}

/// Load a project the caller needs at least `need` on: 404 when they cannot read it, 403 when they can
/// read it but lack the role.
pub async fn load(state: &AppState, owner: &str, slug: &str, user: Option<Uuid>, need: Role) -> ApiResult<(Project, Role)> {
    let project = find(state, owner, slug).await?.ok_or_else(AppError::not_found)?;
    let role = project_role(state, user, &project).await?.ok_or_else(AppError::not_found)?;
    if role < need {
        return Err(AppError::forbidden(format!("this needs the {need} role on {}", project.full_name())));
    }
    Ok((project, role))
}
