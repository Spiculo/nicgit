// SPDX-License-Identifier: AGPL-3.0-or-later
//! In-app notifications.
use serde_json::Value;
use sqlx::PgExecutor;
use uuid::Uuid;

pub async fn send<'e>(db: impl PgExecutor<'e>, user: Uuid, kind: &str, data: Value) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO notifications (id, user_id, kind, data) VALUES ($1, $2, $3, $4)")
        .bind(Uuid::new_v4())
        .bind(user)
        .bind(kind)
        .bind(data)
        .execute(db)
        .await?;
    Ok(())
}
