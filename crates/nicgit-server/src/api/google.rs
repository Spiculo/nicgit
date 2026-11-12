// SPDX-License-Identifier: AGPL-3.0-or-later
//! Sign-in with Google: the OpenID Connect authorization code flow with PKCE (decision N28).
//!
//! The id_token comes straight from Google's token endpoint over TLS, so its claims are checked and its
//! signature is not (OpenID Connect Core 3.1.3.7, item 6). The account's identity is Google's `sub`; an email
//! only links an existing account the first time, and only when Google says it is verified.
use crate::{app::AppState, audit, auth};
use axum::{
    extract::{Query, State},
    response::Redirect,
    routing::get,
    Router,
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

const COOKIE: &str = "nicgit_oauth";
const COOKIE_PATH: &str = "/api/auth/google";
const COOKIE_MINUTES: i64 = 10;
const ISSUERS: [&str; 2] = ["accounts.google.com", "https://accounts.google.com"];

pub fn router() -> Router<AppState> {
    Router::new().route("/api/auth/google/start", get(start)).route("/api/auth/google/callback", get(callback))
}

/// Where a failed sign-in lands. Codes only: nothing Google or a caller wrote reaches the page.
fn failed(code: &str) -> Redirect {
    Redirect::to(&format!("/login?error={code}"))
}

fn redirect_uri(state: &AppState) -> String {
    format!("{}/api/auth/google/callback", state.cfg.public_url.trim_end_matches('/'))
}

fn forget(jar: CookieJar) -> CookieJar {
    jar.remove(Cookie::build(COOKIE).path(COOKIE_PATH).build())
}

async fn start(State(state): State<AppState>, jar: CookieJar) -> (CookieJar, Redirect) {
    let (Some(client_id), Some(_)) = (&state.cfg.google_client_id, &state.cfg.google_client_secret) else {
        return (jar, failed("not_configured"));
    };
    let (csrf, nonce, verifier) = (auth::new_session_token(), auth::new_session_token(), auth::new_session_token());
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let url = reqwest::Url::parse_with_params(
        &state.cfg.google_auth_url,
        &[
            ("client_id", client_id.as_str()),
            ("response_type", "code"),
            ("scope", "openid email profile"),
            ("redirect_uri", &redirect_uri(&state)),
            ("state", &csrf),
            ("nonce", &nonce),
            ("code_challenge", &challenge),
            ("code_challenge_method", "S256"),
        ],
    );
    let Ok(url) = url else {
        tracing::error!("the Google authorization URL is not valid");
        return (jar, failed("not_configured"));
    };
    let cookie = Cookie::build((COOKIE, format!("{csrf}.{nonce}.{verifier}")))
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(state.cfg.https())
        .path(COOKIE_PATH)
        .max_age(time::Duration::minutes(COOKIE_MINUTES))
        .build();
    (jar.add(cookie), Redirect::to(url.as_str()))
}

#[derive(Deserialize)]
struct Callback {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

async fn callback(State(state): State<AppState>, jar: CookieJar, Query(q): Query<Callback>) -> (CookieJar, Redirect) {
    let saved = jar.get(COOKIE).map(|c| c.value().to_string());
    let jar = forget(jar);
    if q.error.is_some() {
        return (jar, failed("denied"));
    }
    let parts: Option<Vec<String>> = saved.map(|s| s.split('.').map(str::to_string).collect());
    let Some([csrf, nonce, verifier]) = parts.and_then(|p| <[String; 3]>::try_from(p).ok()) else {
        return (jar, failed("state"));
    };
    let (Some(code), true) = (q.code, q.state.is_some_and(|s| same(&s, &csrf))) else {
        return (jar, failed("state"));
    };
    let identity = match exchange(&state, &code, &verifier).await.and_then(|claims| check_claims(&claims, &state, &nonce)) {
        Ok(i) => i,
        Err(code) => return (jar, failed(code)),
    };
    let user = match account(&state, &identity).await {
        Ok(u) => u,
        Err(code) => return (jar, failed(code)),
    };
    match crate::api::auth::start_session(&state, jar.clone(), user).await {
        Ok(jar) => (jar, Redirect::to("/")),
        Err(e) => {
            tracing::error!("starting a session: {}", e.message);
            (jar, failed("server"))
        }
    }
}

/// Compare digests so the comparison takes the same time whatever the input.
fn same(a: &str, b: &str) -> bool {
    Sha256::digest(a.as_bytes()) == Sha256::digest(b.as_bytes())
}

/// Trade the code for the id_token's claims.
async fn exchange(state: &AppState, code: &str, verifier: &str) -> Result<Value, &'static str> {
    let (Some(id), Some(secret)) = (&state.cfg.google_client_id, &state.cfg.google_client_secret) else {
        return Err("not_configured");
    };
    let answer = reqwest::Client::new()
        .post(&state.cfg.google_token_url)
        .timeout(std::time::Duration::from_secs(15))
        .form(&[
            ("code", code),
            ("client_id", id),
            ("client_secret", secret),
            ("redirect_uri", &redirect_uri(state)),
            ("grant_type", "authorization_code"),
            ("code_verifier", verifier),
        ])
        .send()
        .await
        .map_err(|e| {
            tracing::warn!("Google token endpoint: {}", e.without_url());
            "exchange"
        })?;
    if !answer.status().is_success() {
        tracing::warn!("Google token endpoint answered {}", answer.status());
        return Err("exchange");
    }
    let body: Value = answer.json().await.map_err(|_| "exchange")?;
    let payload = body["id_token"].as_str().and_then(|t| t.split('.').nth(1)).ok_or("token")?;
    let bytes = URL_SAFE_NO_PAD.decode(payload).map_err(|_| "token")?;
    serde_json::from_slice(&bytes).map_err(|_| "token")
}

struct Identity {
    sub: String,
    email: String,
    name: String,
}

fn check_claims(claims: &Value, state: &AppState, nonce: &str) -> Result<Identity, &'static str> {
    let text = |k: &str| claims[k].as_str().unwrap_or("");
    let now = chrono::Utc::now().timestamp();
    let valid = ISSUERS.contains(&text("iss"))
        && Some(text("aud")) == state.cfg.google_client_id.as_deref()
        && claims["exp"].as_i64().is_some_and(|exp| exp > now)
        && same(text("nonce"), nonce)
        && !text("sub").is_empty();
    if !valid {
        return Err("token");
    }
    // Absent means no: only an address Google vouches for can name an account or the site admin.
    if claims["email_verified"] != json!(true) && claims["email_verified"] != json!("true") {
        return Err("unverified");
    }
    let email = text("email").trim().to_lowercase();
    if !email.contains('@') {
        return Err("unverified");
    }
    Ok(Identity { sub: text("sub").to_string(), email, name: text("name").trim().to_string() })
}

fn is_admin(state: &AppState, email: &str) -> bool {
    state.cfg.site_admin_email.as_deref().is_some_and(|a| a.trim().eq_ignore_ascii_case(email))
}

fn server_error<E: std::fmt::Display>(e: E) -> &'static str {
    tracing::error!("signing in: {e}");
    "server"
}

/// The account for this Google identity: known by `sub`, else linked by verified email, else new when this
/// server lets that address join (decision N39).
async fn account(state: &AppState, who: &Identity) -> Result<Uuid, &'static str> {
    let admin = is_admin(state, &who.email);
    let mut tx = state.db.begin().await.map_err(server_error)?;
    let known: Option<Uuid> = sqlx::query_scalar("SELECT account_id FROM users WHERE google_sub = $1")
        .bind(&who.sub)
        .fetch_optional(&mut *tx)
        .await
        .map_err(server_error)?;
    let (id, created) = if let Some(id) = known {
        // Follow Google's address, unless another account already has it.
        sqlx::query("UPDATE users SET email = $2 WHERE account_id = $1 AND NOT EXISTS (SELECT 1 FROM users WHERE email = $2 AND account_id <> $1)")
            .bind(id)
            .bind(&who.email)
            .execute(&mut *tx)
            .await
            .map_err(server_error)?;
        (id, false)
    } else {
        let by_email: Option<(Uuid, Option<String>)> = sqlx::query_as("SELECT account_id, google_sub FROM users WHERE email = $1")
            .bind(&who.email)
            .fetch_optional(&mut *tx)
            .await
            .map_err(server_error)?;
        match by_email {
            Some((_, Some(_))) => return Err("conflict"),
            Some((id, None)) => {
                sqlx::query("UPDATE users SET google_sub = $2 WHERE account_id = $1").bind(id).bind(&who.sub).execute(&mut *tx).await.map_err(server_error)?;
                (id, false)
            }
            None if !state.cfg.signup.allows(&who.email, admin) => return Err("signup_closed"),
            None => (create(&mut tx, who).await?, true),
        }
    };
    sqlx::query("UPDATE users SET is_site_admin = $2 WHERE account_id = $1").bind(id).bind(admin).execute(&mut *tx).await.map_err(server_error)?;
    let slug: String = sqlx::query_scalar("SELECT slug FROM accounts WHERE id = $1").bind(id).fetch_one(&mut *tx).await.map_err(server_error)?;
    let action = if created { "user.signup" } else { "user.login" };
    audit::record(&mut *tx, Some(id), Some(id), action, &slug, json!({})).await.map_err(server_error)?;
    tx.commit().await.map_err(|_| "conflict")?;
    Ok(id)
}

/// A few random characters, for a person whose name cannot become a user name.
fn id_hint() -> String {
    Uuid::new_v4().simple().to_string()[..8].to_string()
}

async fn create(tx: &mut sqlx::Transaction<'_, sqlx::Postgres>, who: &Identity) -> Result<Uuid, &'static str> {
    // From the name on the Google account, never the email address: user names are public (decision N34).
    let base = nicgit_core::names::slug_base(&who.name).unwrap_or_else(|| format!("user-{}", &id_hint()));
    let id = Uuid::new_v4();
    let mut taken = true;
    for n in 1..=1000 {
        let slug = if n == 1 { base.clone() } else { format!("{base}-{n}") };
        if !nicgit_core::names::valid_slug(&slug) {
            continue;
        }
        let display = if who.name.is_empty() { slug.as_str() } else { who.name.as_str() };
        let inserted = sqlx::query(
            "INSERT INTO accounts (id, kind, slug, display_name) SELECT $1, 'user', $2, $3 \
             WHERE NOT EXISTS (SELECT 1 FROM retired_slugs WHERE slug = $2) ON CONFLICT (slug) DO NOTHING",
        )
            .bind(id)
            .bind(&slug)
            .bind(display)
            .execute(&mut **tx)
            .await
            .map_err(server_error)?;
        if inserted.rows_affected() == 1 {
            taken = false;
            break;
        }
    }
    if taken {
        return Err("server");
    }
    // A unique violation here is the same person signing in twice at once.
    sqlx::query("INSERT INTO users (account_id, email, google_sub) VALUES ($1, $2, $3)")
        .bind(id)
        .bind(&who.email)
        .bind(&who.sub)
        .execute(&mut **tx)
        .await
        .map_err(|_| "conflict")?;
    Ok(id)
}

/// Make the site admin flag match the configuration: run at every start, so removing the address demotes.
pub async fn sync_site_admin(db: &PgPool, email: Option<&str>) -> anyhow::Result<()> {
    let email = email.map(|e| e.trim().to_lowercase());
    sqlx::query("UPDATE users SET is_site_admin = COALESCE(email = $1, false)").bind(email).execute(db).await?;
    Ok(())
}
