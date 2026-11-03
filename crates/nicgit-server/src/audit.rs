// SPDX-License-Identifier: AGPL-3.0-or-later
//! Per-account audit log (spec section 7): who did what to whose data.
use serde_json::Value;
use sqlx::PgExecutor;
use uuid::Uuid;

pub async fn record<'e>(
    db: impl PgExecutor<'e>,
    account: Option<Uuid>,
    actor: Option<Uuid>,
    action: &str,
    target: &str,
    data: Value,
) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO audit_log (account_id, actor_id, action, target, data) VALUES ($1, $2, $3, $4, $5)")
        .bind(account)
        .bind(actor)
        .bind(action)
        .bind(target)
        .bind(data)
        .execute(db)
        .await?;
    Ok(())
}
