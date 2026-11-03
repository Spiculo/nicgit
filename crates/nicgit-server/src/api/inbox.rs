// SPDX-License-Identifier: AGPL-3.0-or-later
//! What is waiting for the signed-in user: proposals to review and recent notifications.
use crate::{app::AppState, auth::CurrentUser, error::ApiResult};
use axum::{extract::State, routing::{get, post}, Json, Router};
use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new().route("/api/me/inbox", get(inbox)).route("/api/me/notifications/read", post(mark_read))
}

async fn inbox(State(state): State<AppState>, user: CurrentUser) -> ApiResult<Json<Value>> {
    // Open proposals in projects where the user may review, not theirs, without a live review of theirs.
    let reviews: Vec<(String, String, i32, String)> = sqlx::query_as(
        "SELECT a.slug, pr.slug, p.number, p.title FROM proposals p \
         JOIN projects pr ON pr.id = p.project_id JOIN accounts a ON a.id = pr.owner_id \
         WHERE p.state = 'open' AND p.author_id <> $1 \
           AND (EXISTS (SELECT 1 FROM project_members m WHERE m.project_id = pr.id AND m.user_id = $1 AND m.role IN ('reviewer','lead','admin')) \
             OR EXISTS (SELECT 1 FROM org_members o WHERE o.org_id = pr.owner_id AND o.user_id = $1 AND o.role IN ('reviewer','lead','admin')) \
             OR (a.kind = 'user' AND pr.owner_id = $1)) \
           AND NOT EXISTS (SELECT 1 FROM reviews r WHERE r.proposal_id = p.id AND r.user_id = $1 AND NOT r.dismissed) \
         ORDER BY p.created_at DESC LIMIT 50",
    )
    .bind(user.id)
    .fetch_all(&state.db)
    .await?;
    let notes: Vec<(Uuid, String, Value, bool, DateTime<Utc>)> =
        sqlx::query_as("SELECT id, kind, data, read, created_at FROM notifications WHERE user_id = $1 ORDER BY created_at DESC LIMIT 30")
            .bind(user.id)
            .fetch_all(&state.db)
            .await?;
    Ok(Json(json!({
        "reviews": reviews.into_iter().map(|(o, s, n, t)| json!({"project": format!("{o}/{s}"), "number": n, "title": t})).collect::<Vec<_>>(),
        "notifications": notes.into_iter().map(|(id, k, d, r, at)| json!({"id": id, "kind": k, "data": d, "read": r, "created_at": at})).collect::<Vec<_>>(),
    })))
}

async fn mark_read(State(state): State<AppState>, user: CurrentUser) -> ApiResult<Json<Value>> {
    sqlx::query("UPDATE notifications SET read = true WHERE user_id = $1 AND NOT read").bind(user.id).execute(&state.db).await?;
    Ok(Json(json!({"ok": true})))
}
