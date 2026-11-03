// SPDX-License-Identifier: AGPL-3.0-or-later
//! The project activity feed.
use serde_json::Value;
use sqlx::PgExecutor;
use uuid::Uuid;

pub async fn record<'e>(db: impl PgExecutor<'e>, project: Uuid, actor: Option<Uuid>, kind: &str, data: Value) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO activity (project_id, actor_id, kind, data) VALUES ($1, $2, $3, $4)")
        .bind(project)
        .bind(actor)
        .bind(kind)
        .bind(data)
        .execute(db)
        .await?;
    Ok(())
}
