// SPDX-License-Identifier: AGPL-3.0-or-later
use crate::{
    app::AppState,
    audit,
    auth::{self, CurrentUser, SESSION_COOKIE},
    error::{ApiResult, AppError},
};
use axum::{
    extract::{Path, Query, State},
    response::Redirect,
    routing::{delete, get, post},
    Json, Router,
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use time::Duration;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/dev/login", get(dev_login))
        .route("/api/auth/logout", post(logout))
        .route("/api/me", get(me).patch(update_me))
        .route("/api/me/onboarding", post(onboarding))
        .route("/api/me/tokens", get(list_tokens).post(create_token))
        .route("/api/me/tokens/{id}", delete(delete_token))
}

pub(crate) async fn start_session(state: &AppState, jar: CookieJar, user: Uuid) -> ApiResult<CookieJar> {
    let token = auth::new_session_token();
    sqlx::query("INSERT INTO sessions (id, user_id, expires_at) VALUES ($1, $2, now() + make_interval(days => $3))")
        .bind(auth::sha256_hex(&token))
        .bind(user)
        .bind(auth::SESSION_DAYS as i32)
        .execute(&state.db)
        .await?;
    let cookie = Cookie::build((SESSION_COOKIE, token))
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(state.cfg.https())
        .path("/")
        .max_age(Duration::days(auth::SESSION_DAYS))
        .build();
    Ok(jar.add(cookie))
}

#[derive(Deserialize)]
struct DevLogin {
    slug: Option<String>,
}

/// Local preview only: sign in as a dev account without Google. Enabled by NICGIT_DEV=1 and never
/// reachable otherwise (the same switch `dev-user` uses).
async fn dev_login(State(state): State<AppState>, jar: CookieJar, Query(q): Query<DevLogin>) -> ApiResult<(CookieJar, Redirect)> {
    if std::env::var("NICGIT_DEV").ok().as_deref() != Some("1") {
        return Err(AppError::not_found());
    }
    let slug = q.slug.unwrap_or_else(|| "ana".into());
    if !nicgit_core::names::valid_slug(&slug) {
        return Err(AppError::bad_request("not a valid user name"));
    }
    let id = crate::dev::create_user(&state.db, &slug, &format!("{slug}@example.com"), &slug).await?;
    let jar = start_session(&state, jar, id).await?;
    Ok((jar, Redirect::to("/")))
}

async fn logout(State(state): State<AppState>, jar: CookieJar) -> ApiResult<(CookieJar, Json<Value>)> {
    if let Some(c) = jar.get(SESSION_COOKIE) {
        sqlx::query("DELETE FROM sessions WHERE id = $1").bind(auth::sha256_hex(c.value())).execute(&state.db).await?;
    }
    Ok((jar.remove(Cookie::build(SESSION_COOKIE).path("/").build()), Json(json!({"ok": true}))))
}

async fn me(State(state): State<AppState>, user: CurrentUser) -> ApiResult<Json<Value>> {
    let r: MeRow = sqlx::query_as(
        "SELECT u.email, a.display_name, u.lang, u.style, (u.onboarded_at IS NOT NULL) AS onboarded, \
                u.work, u.years_experience, u.site_experience, u.focus, u.company, u.team_size, u.company_size \
         FROM users u JOIN accounts a ON a.id = u.account_id WHERE u.account_id = $1",
    )
    .bind(user.id)
    .fetch_one(&state.db)
    .await?;
    let orgs: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT a.slug, a.display_name, m.role FROM org_members m JOIN accounts a ON a.id = m.org_id \
         WHERE m.user_id = $1 ORDER BY a.slug",
    )
    .bind(user.id)
    .fetch_all(&state.db)
    .await?;
    let orgs: Vec<Value> =
        orgs.into_iter().map(|(slug, name, role)| json!({"slug": slug, "display_name": name, "role": role})).collect();
    Ok(Json(json!({
        "id": user.id, "slug": user.slug, "email": r.email, "display_name": r.display_name, "lang": r.lang,
        "style": r.style, "onboarded": r.onboarded, "work": r.work, "years_experience": r.years_experience,
        "site_experience": r.site_experience, "focus": r.focus, "company": r.company,
        "team_size": r.team_size, "company_size": r.company_size,
        "is_site_admin": user.is_site_admin, "orgs": orgs,
    })))
}

#[derive(sqlx::FromRow)]
struct MeRow {
    email: String,
    display_name: String,
    lang: String,
    style: String,
    onboarded: bool,
    work: String,
    years_experience: Option<i32>,
    site_experience: String,
    focus: String,
    company: String,
    team_size: String,
    company_size: String,
}

const WORK: [&str; 7] = ["architect", "engineer", "contractor", "manager", "client", "student", "other"];
const SITE: [&str; 4] = ["none", "little", "regular", "extensive"];
const FOCUS: [&str; 7] = ["residential", "commercial", "public", "infrastructure", "interiors", "education", "other"];
const TEAM_SIZE: [&str; 5] = ["solo", "two_to_five", "six_to_twenty", "twenty_one_to_fifty", "fifty_plus"];
const COMPANY_SIZE: [&str; 5] = ["one_to_ten", "eleven_to_fifty", "fifty_one_to_two_hundred", "two_hundred_one_to_thousand", "thousand_plus"];

fn allowed(value: &str, accepted: &[&str], field: &str) -> ApiResult<()> {
    if accepted.contains(&value) {
        Ok(())
    } else {
        Err(AppError::bad_request(format!("choose a valid {field}")))
    }
}

#[derive(Deserialize)]
struct Onboarding {
    lang: String,
    style: String,
    work: String,
    years_experience: Option<i32>,
    site_experience: String,
    #[serde(default)]
    focus: String,
    #[serde(default)]
    company: String,
    #[serde(default)]
    team_size: String,
    #[serde(default)]
    company_size: String,
}

/// The first-run answers. One transaction so the language, wording and profile change together.
async fn onboarding(State(state): State<AppState>, user: CurrentUser, Json(b): Json<Onboarding>) -> ApiResult<Json<Value>> {
    if !["en", "es"].contains(&b.lang.as_str()) {
        return Err(AppError::bad_request("language must be en or es"));
    }
    if !["git", "architecture"].contains(&b.style.as_str()) {
        return Err(AppError::bad_request("style must be git or architecture"));
    }
    allowed(b.work.trim(), &WORK, "field")?;
    allowed(b.site_experience.trim(), &SITE, "site experience")?;
    if !b.focus.trim().is_empty() {
        allowed(b.focus.trim(), &FOCUS, "project focus")?;
    }
    if !b.team_size.trim().is_empty() {
        allowed(b.team_size.trim(), &TEAM_SIZE, "team size")?;
    }
    if !b.company_size.trim().is_empty() {
        allowed(b.company_size.trim(), &COMPANY_SIZE, "company size")?;
    }
    let years = b.years_experience.ok_or_else(|| AppError::bad_request("add your years in the field"))?;
    if !(0..=80).contains(&years) {
        return Err(AppError::bad_request("years in the field must be between 0 and 80"));
    }
    let company = b.company.trim();
    if company.chars().count() > 120 {
        return Err(AppError::bad_request("the company name is too long"));
    }
    let work = b.work.trim();
    let site = b.site_experience.trim();
    let focus = b.focus.trim();
    let team_size = b.team_size.trim();
    let company_size = b.company_size.trim();
    sqlx::query(
        "UPDATE users SET lang = $2, style = $3, work = $4, years_experience = $5, site_experience = $6, \
                focus = $7, company = $8, team_size = $9, company_size = $10, onboarded_at = COALESCE(onboarded_at, now()) \
         WHERE account_id = $1",
    )
    .bind(user.id)
    .bind(&b.lang)
    .bind(&b.style)
    .bind(work)
    .bind(years)
    .bind(site)
    .bind(focus)
    .bind(company)
    .bind(team_size)
    .bind(company_size)
    .execute(&state.db)
    .await?;
    audit::record(&state.db, Some(user.id), Some(user.id), "user.onboarding", &user.slug, json!({
        "work": work, "years_experience": years, "site_experience": site, "focus": focus,
        "team_size": team_size, "company_size": company_size,
    }))
    .await?;
    Ok(Json(json!({"ok": true})))
}

#[derive(Deserialize)]
struct MeUpdate {
    display_name: Option<String>,
    lang: Option<String>,
    style: Option<String>,
    slug: Option<String>,
}

/// Names an account has given up stay reserved, so this bounds how many one person can take out of use.
const MAX_RETIRED_SLUGS: i64 = 3;

/// Change a user name. Projects keep working because everything refers to the account's id, but addresses
/// under the old name stop, and nobody else can ever take it (decision N34).
async fn rename(state: &AppState, user: &CurrentUser, slug: &str) -> ApiResult<()> {
    if slug == user.slug {
        return Ok(());
    }
    if !nicgit_core::names::valid_slug(slug) {
        return Err(AppError::bad_request("user names use lowercase letters, digits, '-', '_' or '.'"));
    }
    let taken = || AppError::conflict("taken", "that name is already in use");
    let mut tx = state.db.begin().await?;
    let retired_by: Option<uuid::Uuid> = sqlx::query_scalar("SELECT account_id FROM retired_slugs WHERE slug = $1").bind(slug).fetch_optional(&mut *tx).await?;
    if retired_by.is_some_and(|by| by != user.id) {
        return Err(taken());
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM retired_slugs WHERE account_id = $1").bind(user.id).fetch_one(&mut *tx).await?;
    if retired_by.is_none() && count >= MAX_RETIRED_SLUGS {
        return Err(AppError::forbidden("you have changed your user name as many times as allowed"));
    }
    sqlx::query("DELETE FROM retired_slugs WHERE slug = $1").bind(slug).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO retired_slugs (slug, account_id) VALUES ($1, $2)").bind(&user.slug).bind(user.id).execute(&mut *tx).await?;
    let updated = sqlx::query("UPDATE accounts SET slug = $2 WHERE id = $1 AND NOT EXISTS (SELECT 1 FROM accounts WHERE slug = $2)")
        .bind(user.id)
        .bind(slug)
        .execute(&mut *tx)
        .await
        .map_err(|_| taken())?;
    if updated.rows_affected() == 0 {
        return Err(taken());
    }
    audit::record(&mut *tx, Some(user.id), Some(user.id), "user.rename", slug, json!({"from": user.slug})).await?;
    tx.commit().await?;
    Ok(())
}

async fn update_me(State(state): State<AppState>, user: CurrentUser, Json(b): Json<MeUpdate>) -> ApiResult<Json<Value>> {
    if let Some(slug) = &b.slug {
        rename(&state, &user, slug).await?;
    }
    if let Some(lang) = &b.lang {
        if !["en", "es"].contains(&lang.as_str()) {
            return Err(AppError::bad_request("language must be en or es"));
        }
        sqlx::query("UPDATE users SET lang = $2 WHERE account_id = $1").bind(user.id).bind(lang).execute(&state.db).await?;
    }
    if let Some(style) = &b.style {
        if !["git", "architecture"].contains(&style.as_str()) {
            return Err(AppError::bad_request("style must be git or architecture"));
        }
        sqlx::query("UPDATE users SET style = $2 WHERE account_id = $1").bind(user.id).bind(style).execute(&state.db).await?;
    }
    if let Some(name) = &b.display_name {
        sqlx::query("UPDATE accounts SET display_name = $2 WHERE id = $1").bind(user.id).bind(name.trim()).execute(&state.db).await?;
    }
    Ok(Json(json!({"ok": true})))
}

#[derive(Deserialize)]
struct NewToken {
    name: String,
}

async fn create_token(State(state): State<AppState>, user: CurrentUser, Json(b): Json<NewToken>) -> ApiResult<Json<Value>> {
    let name = b.name.trim();
    if name.is_empty() || name.len() > 100 {
        return Err(AppError::bad_request("give the token a name"));
    }
    let token = auth::new_pat();
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO tokens (id, user_id, name, token_hash) VALUES ($1, $2, $3, $4)")
        .bind(id)
        .bind(user.id)
        .bind(name)
        .bind(auth::sha256_hex(&token))
        .execute(&state.db)
        .await?;
    audit::record(&state.db, Some(user.id), Some(user.id), "token.create", name, json!({"id": id})).await?;
    Ok(Json(json!({"id": id, "name": name, "token": token})))
}

async fn list_tokens(State(state): State<AppState>, user: CurrentUser) -> ApiResult<Json<Value>> {
    let rows: Vec<(Uuid, String, Option<DateTime<Utc>>, DateTime<Utc>)> = sqlx::query_as(
        "SELECT id, name, last_used_at, created_at FROM tokens WHERE user_id = $1 ORDER BY created_at DESC",
    )
    .bind(user.id)
    .fetch_all(&state.db)
    .await?;
    let out: Vec<Value> = rows
        .into_iter()
        .map(|(id, name, used, created)| json!({"id": id, "name": name, "last_used_at": used, "created_at": created}))
        .collect();
    Ok(Json(json!(out)))
}

async fn delete_token(State(state): State<AppState>, user: CurrentUser, Path(id): Path<Uuid>) -> ApiResult<Json<Value>> {
    let n = sqlx::query("DELETE FROM tokens WHERE id = $1 AND user_id = $2").bind(id).bind(user.id).execute(&state.db).await?;
    if n.rows_affected() == 0 {
        return Err(AppError::not_found());
    }
    audit::record(&state.db, Some(user.id), Some(user.id), "token.delete", "", json!({"id": id})).await?;
    Ok(Json(json!({"ok": true})))
}
