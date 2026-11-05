// SPDX-License-Identifier: AGPL-3.0-or-later
//! Accounts without Google, for tests and the `dev-user` command. Not reachable over the network.
use crate::{audit, auth};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

/// The account `slug`, made if it does not exist yet.
pub async fn create_user(db: &PgPool, slug: &str, email: &str, display_name: &str) -> anyhow::Result<Uuid> {
    anyhow::ensure!(nicgit_core::names::valid_slug(slug), "not a valid user name: {slug}");
    let email = email.trim().to_lowercase();
    let mut tx = db.begin().await?;
    if let Some(id) = sqlx::query_scalar("SELECT account_id FROM users u JOIN accounts a ON a.id = u.account_id WHERE a.slug = $1").bind(slug).fetch_optional(&mut *tx).await? {
        return Ok(id);
    }
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO accounts (id, kind, slug, display_name) VALUES ($1, 'user', $2, $3)").bind(id).bind(slug).bind(display_name).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO users (account_id, email) VALUES ($1, $2)").bind(id).bind(&email).execute(&mut *tx).await?;
    audit::record(&mut *tx, Some(id), Some(id), "user.signup", slug, json!({"via": "dev"})).await?;
    tx.commit().await?;
    Ok(id)
}

/// A session cookie value.
pub async fn new_session(db: &PgPool, user: Uuid) -> anyhow::Result<String> {
    let token = auth::new_session_token();
    sqlx::query("INSERT INTO sessions (id, user_id, expires_at) VALUES ($1, $2, now() + make_interval(days => $3))")
        .bind(auth::sha256_hex(&token))
        .bind(user)
        .bind(auth::SESSION_DAYS as i32)
        .execute(db)
        .await?;
    Ok(token)
}

/// A personal access token.
pub async fn new_token(db: &PgPool, user: Uuid, name: &str) -> anyhow::Result<String> {
    let token = auth::new_pat();
    sqlx::query("INSERT INTO tokens (id, user_id, name, token_hash) VALUES ($1, $2, $3, $4)")
        .bind(Uuid::new_v4())
        .bind(user)
        .bind(name)
        .bind(auth::sha256_hex(&token))
        .execute(db)
        .await?;
    Ok(token)
}
