// SPDX-License-Identifier: AGPL-3.0-or-later
use crate::{app::AppState, error::AppError};
use axum::{extract::FromRequestParts, http::request::Parts};
use axum_extra::extract::CookieJar;
use base64::Engine;
use uuid::Uuid;

pub const SESSION_COOKIE: &str = "nicgit_session";

/// The signed-in user (session cookie, `Bearer <pat>`, or HTTP Basic with a PAT as password).
#[derive(Debug, Clone)]
pub struct CurrentUser {
    pub id: Uuid,
    pub slug: String,
    pub is_site_admin: bool,
}

/// The signed-in user, if any.
#[derive(Debug, Clone)]
pub struct MaybeUser(pub Option<CurrentUser>);

fn credential(parts: &Parts) -> Option<(bool, String)> {
    // (is_pat, secret)
    if let Some(h) = parts.headers.get(axum::http::header::AUTHORIZATION).and_then(|v| v.to_str().ok()) {
        if let Some(t) = h.strip_prefix("Bearer ") {
            return Some((true, t.trim().to_string()));
        }
        if let Some(b) = h.strip_prefix("Basic ") {
            let decoded = base64::engine::general_purpose::STANDARD.decode(b.trim()).ok()?;
            let s = String::from_utf8(decoded).ok()?;
            let (_, pass) = s.split_once(':')?;
            return Some((true, pass.to_string()));
        }
    }
    let jar = CookieJar::from_headers(&parts.headers);
    jar.get(SESSION_COOKIE).map(|c| (false, c.value().to_string()))
}

pub async fn lookup(state: &AppState, parts: &Parts) -> Result<Option<CurrentUser>, AppError> {
    let Some((is_pat, secret)) = credential(parts) else { return Ok(None) };
    let hash = super::sha256_hex(&secret);
    let row: Option<(Uuid, String, bool)> = if is_pat {
        sqlx::query_as(
            "UPDATE tokens t SET last_used_at = now() FROM users u JOIN accounts a ON a.id = u.account_id \
             WHERE t.token_hash = $1 AND u.account_id = t.user_id RETURNING a.id, a.slug, u.is_site_admin",
        )
        .bind(&hash)
        .fetch_optional(&state.db)
        .await?
    } else {
        sqlx::query_as(
            "SELECT a.id, a.slug, u.is_site_admin FROM sessions s JOIN users u ON u.account_id = s.user_id \
             JOIN accounts a ON a.id = u.account_id WHERE s.id = $1 AND s.expires_at > now()",
        )
        .bind(&hash)
        .fetch_optional(&state.db)
        .await?
    };
    Ok(row.map(|(id, slug, is_site_admin)| CurrentUser { id, slug, is_site_admin }))
}

impl FromRequestParts<AppState> for MaybeUser {
    type Rejection = AppError;
    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        if let Some(u) = parts.extensions.get::<MaybeUser>() {
            return Ok(u.clone());
        }
        let u = MaybeUser(lookup(state, parts).await?);
        parts.extensions.insert(u.clone());
        Ok(u)
    }
}

impl FromRequestParts<AppState> for CurrentUser {
    type Rejection = AppError;
    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        MaybeUser::from_request_parts(parts, state).await?.0.ok_or_else(AppError::unauthorized)
    }
}
