// SPDX-License-Identifier: AGPL-3.0-or-later
//! Rules on protected design options (spec section 9).
use nicgit_core::conflicts::ConflictMode;
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct OptionRule {
    pub option: String,
    pub required_approvals: i32,
    pub required_roles: Vec<String>,
    pub allowed_modes: Vec<String>,
    pub required_checks: Vec<String>,
    pub protected_settings: Vec<String>,
    pub dismiss_on_push: bool,
}

impl OptionRule {
    /// The rule of an unprotected option: anything goes, no approvals needed.
    pub fn open(option: &str) -> OptionRule {
        OptionRule {
            option: option.into(),
            required_approvals: 0,
            required_roles: vec![],
            allowed_modes: ConflictMode::ALL.iter().map(|m| m.as_str().to_string()).collect(),
            required_checks: vec![],
            protected_settings: vec![],
            dismiss_on_push: true,
        }
    }

    pub fn allows(&self, mode: ConflictMode) -> bool {
        self.allowed_modes.iter().any(|m| m == mode.as_str())
    }
}

pub async fn protected_options(db: &PgPool, project: Uuid) -> Result<Vec<String>, crate::error::AppError> {
    Ok(sqlx::query_scalar("SELECT option FROM option_rules WHERE project_id = $1 ORDER BY option").bind(project).fetch_all(db).await?)
}

pub async fn rule_for(db: &PgPool, project: Uuid, option: &str) -> Result<Option<OptionRule>, crate::error::AppError> {
    Ok(sqlx::query_as(
        "SELECT option, required_approvals, required_roles, allowed_modes, required_checks, protected_settings, dismiss_on_push \
         FROM option_rules WHERE project_id = $1 AND option = $2",
    )
    .bind(project)
    .bind(option)
    .fetch_optional(db)
    .await?)
}
