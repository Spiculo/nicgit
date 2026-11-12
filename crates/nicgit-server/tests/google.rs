// SPDX-License-Identifier: AGPL-3.0-or-later
//! Google sign-in against a mock Google: the claims in the id_token decide everything.
mod common;
use axum::{extract::State, routing::post, Form, Json, Router};
use base64::Engine;
use nicgit_server::config::Signup;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

const CLIENT_ID: &str = "test-client";

#[derive(Clone, Default)]
struct Mock {
    claims: Arc<Mutex<Value>>,
    form: Arc<Mutex<HashMap<String, String>>>,
}

async fn token(State(m): State<Mock>, Form(form): Form<HashMap<String, String>>) -> Json<Value> {
    *m.form.lock().unwrap() = form;
    let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(m.claims.lock().unwrap().to_string());
    Json(json!({"access_token": "x", "id_token": format!("e30.{payload}.sig")}))
}

async fn setup(admin: Option<&str>) -> (common::TestApp, Mock) {
    setup_signup(admin, Signup { open: true, domains: vec![] }).await
}

/// A server whose sign-up is closed except for `domains` (a self-hosted studio's server).
async fn setup_closed(admin: Option<&str>, domains: &[&str]) -> (common::TestApp, Mock) {
    setup_signup(admin, Signup { open: false, domains: domains.iter().map(|d| d.to_string()).collect() }).await
}

async fn setup_signup(admin: Option<&str>, signup: Signup) -> (common::TestApp, Mock) {
    let mock = Mock::default();
    let router = Router::new().route("/token", post(token)).with_state(mock.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let admin = admin.map(str::to_string);
    let app = common::TestApp::spawn_with(move |cfg| {
        cfg.google_client_id = Some(CLIENT_ID.into());
        cfg.google_client_secret = Some("secret".into());
        cfg.google_auth_url = "https://google.test/auth".into();
        cfg.google_token_url = format!("http://{addr}/token");
        cfg.site_admin_email = admin;
        cfg.signup = signup;
    })
    .await;
    (app, mock)
}

fn browser() -> reqwest::Client {
    reqwest::Client::builder().cookie_store(true).redirect(reqwest::redirect::Policy::none()).build().unwrap()
}

fn good_claims(sub: &str, email: &str) -> Value {
    json!({
        "iss": "https://accounts.google.com", "aud": CLIENT_ID, "exp": 4102444800i64,
        "sub": sub, "email": email, "email_verified": true, "name": "Ana Ferreira",
    })
}

fn location(r: &reqwest::Response) -> String {
    r.headers()["location"].to_str().unwrap().to_string()
}

/// The whole flow in one browser. `claims` without a nonce get the one the server asked for.
async fn sign_in(app: &common::TestApp, mock: &Mock, c: &reqwest::Client, mut claims: Value) -> String {
    let r = c.get(app.url("/api/auth/google/start")).send().await.unwrap();
    assert!(r.status().is_redirection());
    let url = reqwest::Url::parse(&location(&r)).unwrap();
    let q: HashMap<String, String> = url.query_pairs().into_owned().collect();
    if claims.get("nonce").is_none() {
        claims["nonce"] = json!(q["nonce"]);
    }
    *mock.claims.lock().unwrap() = claims;
    let r = c.get(app.url("/api/auth/google/callback")).query(&[("code", "abc"), ("state", q["state"].as_str())]).send().await.unwrap();
    assert!(r.status().is_redirection());
    location(&r)
}

async fn me(app: &common::TestApp, c: &reqwest::Client) -> Value {
    c.get(app.url("/api/me")).send().await.unwrap().json().await.unwrap()
}

#[tokio::test]
async fn new_user_signs_in() {
    let (app, mock) = setup(None).await;
    let c = browser();
    assert_eq!(sign_in(&app, &mock, &c, good_claims("s1", "Ana@Example.com")).await, "/");
    let v = me(&app, &c).await;
    assert_eq!(v["slug"], "ana-ferreira");
    assert_eq!(v["email"], "ana@example.com");
    assert_eq!(v["display_name"], "Ana Ferreira");
    assert_eq!(v["is_site_admin"], false);
    let form = mock.form.lock().unwrap().clone();
    assert_eq!(form["code"], "abc");
    assert_eq!(form["grant_type"], "authorization_code");
    assert!(form["redirect_uri"].ends_with("/api/auth/google/callback"));
    assert!(!form["code_verifier"].is_empty(), "PKCE");
}

#[tokio::test]
async fn start_sends_pkce_and_scopes() {
    let (app, _) = setup(None).await;
    let r = browser().get(app.url("/api/auth/google/start")).send().await.unwrap();
    let url = reqwest::Url::parse(&location(&r)).unwrap();
    assert_eq!(url.host_str(), Some("google.test"));
    let q: HashMap<String, String> = url.query_pairs().into_owned().collect();
    assert_eq!(q["client_id"], CLIENT_ID);
    assert_eq!(q["response_type"], "code");
    assert_eq!(q["code_challenge_method"], "S256");
    assert_eq!(q["scope"], "openid email profile");
    assert!(q["redirect_uri"].ends_with("/api/auth/google/callback"));
    let cookie = r.headers()["set-cookie"].to_str().unwrap().to_lowercase();
    assert!(cookie.contains("httponly") && cookie.contains("samesite=lax"));
}

#[tokio::test]
async fn state_mismatch_refused() {
    let (app, mock) = setup(None).await;
    let c = browser();
    c.get(app.url("/api/auth/google/start")).send().await.unwrap();
    *mock.claims.lock().unwrap() = good_claims("s1", "ana@example.com");
    let r = c.get(app.url("/api/auth/google/callback")).query(&[("code", "abc"), ("state", "forged")]).send().await.unwrap();
    assert_eq!(location(&r), "/login?error=state");
    assert_eq!(c.get(app.url("/api/me")).send().await.unwrap().status(), 401);
}

#[tokio::test]
async fn callback_without_start_refused() {
    let (app, _) = setup(None).await;
    let c = browser();
    let r = c.get(app.url("/api/auth/google/callback")).query(&[("code", "abc"), ("state", "x")]).send().await.unwrap();
    assert_eq!(location(&r), "/login?error=state");
}

#[tokio::test]
async fn user_cancelling_at_google_is_reported() {
    let (app, _) = setup(None).await;
    let c = browser();
    c.get(app.url("/api/auth/google/start")).send().await.unwrap();
    let r = c.get(app.url("/api/auth/google/callback")).query(&[("error", "access_denied")]).send().await.unwrap();
    assert_eq!(location(&r), "/login?error=denied");
}

async fn refused(mutate: impl FnOnce(&mut Value), expected: &str) {
    let (app, mock) = setup(None).await;
    let c = browser();
    let mut claims = good_claims("s1", "ana@example.com");
    mutate(&mut claims);
    assert_eq!(sign_in(&app, &mock, &c, claims).await, format!("/login?error={expected}"));
    assert_eq!(c.get(app.url("/api/me")).send().await.unwrap().status(), 401);
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM users").fetch_one(&app.db).await.unwrap();
    assert_eq!(n, 0, "no account may be created");
}

#[tokio::test]
async fn nonce_mismatch_refused() {
    refused(|c| c["nonce"] = json!("other"), "token").await;
}

#[tokio::test]
async fn wrong_audience_refused() {
    refused(|c| c["aud"] = json!("another-app"), "token").await;
}

#[tokio::test]
async fn wrong_issuer_refused() {
    refused(|c| c["iss"] = json!("https://evil.example"), "token").await;
}

#[tokio::test]
async fn expired_refused() {
    refused(|c| c["exp"] = json!(1000), "token").await;
}

#[tokio::test]
async fn unverified_email_refused() {
    refused(|c| c["email_verified"] = json!(false), "unverified").await;
}

#[tokio::test]
async fn missing_email_verified_refused() {
    refused(|c| { c.as_object_mut().unwrap().remove("email_verified"); }, "unverified").await;
}

#[tokio::test]
async fn unverified_email_cannot_take_over_or_become_admin() {
    let (app, mock) = setup(Some("boss@example.com")).await;
    let owner = browser();
    sign_in(&app, &mock, &owner, good_claims("s1", "boss@example.com")).await;
    let c = browser();
    let mut claims = good_claims("attacker", "boss@example.com");
    claims["email_verified"] = json!(false);
    assert_eq!(sign_in(&app, &mock, &c, claims).await, "/login?error=unverified");
    assert_eq!(c.get(app.url("/api/me")).send().await.unwrap().status(), 401);
}

#[tokio::test]
async fn sub_is_identity_not_email() {
    let (app, mock) = setup(None).await;
    let c = browser();
    sign_in(&app, &mock, &c, good_claims("s1", "ana@example.com")).await;
    let first = me(&app, &c).await;
    let c2 = browser();
    sign_in(&app, &mock, &c2, good_claims("s1", "ana.new@example.com")).await;
    let second = me(&app, &c2).await;
    assert_eq!(first["id"], second["id"]);
    assert_eq!(second["slug"], "ana-ferreira", "the user name stays");
    assert_eq!(second["email"], "ana.new@example.com");
}

#[tokio::test]
async fn verified_email_links_unclaimed_account() {
    let (app, mock) = setup(None).await;
    let existing = app.user("ana").await;
    let before = me(&app, &existing).await;
    let c = browser();
    assert_eq!(sign_in(&app, &mock, &c, good_claims("s1", "ana@example.com")).await, "/");
    assert_eq!(me(&app, &c).await["id"], before["id"]);
}

#[tokio::test]
async fn email_claimed_by_other_sub_refused() {
    let (app, mock) = setup(None).await;
    let c = browser();
    sign_in(&app, &mock, &c, good_claims("s1", "ana@example.com")).await;
    let other = browser();
    assert_eq!(sign_in(&app, &mock, &other, good_claims("s2", "ana@example.com")).await, "/login?error=conflict");
    assert_eq!(other.get(app.url("/api/me")).send().await.unwrap().status(), 401);
}

#[tokio::test]
async fn email_change_onto_someone_elses_email_keeps_the_old_one() {
    let (app, mock) = setup(None).await;
    let a = browser();
    sign_in(&app, &mock, &a, good_claims("s1", "ana@example.com")).await;
    let b = browser();
    sign_in(&app, &mock, &b, good_claims("s2", "bea@example.com")).await;
    let again = browser();
    assert_eq!(sign_in(&app, &mock, &again, good_claims("s2", "ana@example.com")).await, "/");
    assert_eq!(me(&app, &again).await["email"], "bea@example.com");
}

#[tokio::test]
async fn admin_email_grants_and_config_demotes() {
    let (app, mock) = setup(Some("boss@example.com")).await;
    let c = browser();
    sign_in(&app, &mock, &c, good_claims("s1", "boss@example.com")).await;
    assert_eq!(me(&app, &c).await["is_site_admin"], true);
    let other = browser();
    sign_in(&app, &mock, &other, good_claims("s2", "first-but-not-admin@example.com")).await;
    assert_eq!(me(&app, &other).await["is_site_admin"], false);
    // The config is the only source: taking the address out demotes at the next start.
    nicgit_server::api::google::sync_site_admin(&app.db, None).await.unwrap();
    assert_eq!(me(&app, &c).await["is_site_admin"], false);
    nicgit_server::api::google::sync_site_admin(&app.db, Some("boss@example.com")).await.unwrap();
    assert_eq!(me(&app, &c).await["is_site_admin"], true);
}

#[tokio::test]
async fn admin_email_is_case_insensitive() {
    let (app, mock) = setup(Some("Boss@Example.COM")).await;
    let c = browser();
    sign_in(&app, &mock, &c, good_claims("s1", "boss@example.com")).await;
    assert_eq!(me(&app, &c).await["is_site_admin"], true);
}

#[tokio::test]
async fn first_user_is_not_admin_without_config() {
    let (app, mock) = setup(None).await;
    let c = browser();
    sign_in(&app, &mock, &c, good_claims("s1", "first@example.com")).await;
    assert_eq!(me(&app, &c).await["is_site_admin"], false);
}

fn named(sub: &str, email: &str, name: &str) -> Value {
    let mut claims = good_claims(sub, email);
    claims["name"] = json!(name);
    claims
}

#[tokio::test]
async fn slug_comes_from_the_name_and_never_the_email() {
    let (app, mock) = setup(None).await;
    for (sub, name, slug) in [
        ("s1", "API", "api-2"),
        ("s2", "José Núñez", "jose-nunez"),
        ("s3", "Ana Ferreira", "ana-ferreira"),
        ("s4", "Ana Ferreira", "ana-ferreira-2"),
        ("s5", "Ana Ferreira", "ana-ferreira-3"),
    ] {
        let c = browser();
        let email = format!("private.address.{sub}@example.com");
        assert_eq!(sign_in(&app, &mock, &c, named(sub, &email, name)).await, "/", "{name}");
        assert_eq!(me(&app, &c).await["slug"], slug, "{name}");
    }
}

#[tokio::test]
async fn a_name_that_cannot_be_a_slug_gets_a_neutral_one() {
    let (app, mock) = setup(None).await;
    for (sub, name) in [("s1", ""), ("s2", "山田太郎"), ("s3", "+++")] {
        let c = browser();
        sign_in(&app, &mock, &c, named(sub, &format!("secret.{sub}@example.com"), name)).await;
        let slug = me(&app, &c).await["slug"].as_str().unwrap().to_string();
        assert!(slug.starts_with("user-") && !slug.contains("secret"), "{slug}");
    }
}

#[tokio::test]
async fn slug_avoids_org_names() {
    let (app, mock) = setup(None).await;
    let alice = app.user("alice").await;
    alice.post(app.url("/api/orgs")).json(&json!({"slug": "studio"})).send().await.unwrap();
    let c = browser();
    sign_in(&app, &mock, &c, named("s1", "x@example.com", "Studio")).await;
    assert_eq!(me(&app, &c).await["slug"], "studio-2");
}

#[tokio::test]
async fn slug_avoids_names_given_up_by_someone_else() {
    let (app, mock) = setup(None).await;
    let alice = app.user("ana").await;
    assert_eq!(alice.patch(app.url("/api/me")).json(&json!({"slug": "ana-new"})).send().await.unwrap().status(), 200);
    let c = browser();
    sign_in(&app, &mock, &c, named("s1", "x@example.com", "Ana")).await;
    assert_eq!(me(&app, &c).await["slug"], "ana-2");
}

#[tokio::test]
async fn password_routes_are_gone() {
    let (app, _) = setup(None).await;
    let c = reqwest::Client::new();
    for path in ["/api/auth/signup", "/api/auth/login"] {
        let r = c.post(app.url(path)).json(&json!({"slug": "a", "email": "a@b.c", "login": "a", "password": "correct horse battery"})).send().await.unwrap();
        assert_eq!(r.status(), 404, "{path}");
    }
}

#[tokio::test]
async fn not_configured_explains_on_the_login_page() {
    let app = common::TestApp::spawn().await;
    let r = browser().get(app.url("/api/auth/google/start")).send().await.unwrap();
    assert!(r.status().is_redirection());
    assert_eq!(location(&r), "/login?error=not_configured");
}

#[tokio::test]
async fn signing_in_writes_audit() {
    let (app, mock) = setup(None).await;
    let c = browser();
    sign_in(&app, &mock, &c, good_claims("s1", "ana@example.com")).await;
    sign_in(&app, &mock, &c, good_claims("s1", "ana@example.com")).await;
    let count = |action: &'static str| {
        let db = app.db.clone();
        async move { sqlx::query_scalar::<_, i64>("SELECT count(*) FROM audit_log WHERE action = $1").bind(action).fetch_one(&db).await.unwrap() }
    };
    assert_eq!(count("user.signup").await, 1);
    assert_eq!(count("user.login").await, 1);
}

#[tokio::test]
async fn pat_still_works_for_git_style_auth() {
    let (app, mock) = setup(None).await;
    let c = browser();
    sign_in(&app, &mock, &c, good_claims("s1", "ana@example.com")).await;
    let token = app.token(&c).await;
    let r = reqwest::Client::new().get(app.url("/api/me")).basic_auth("ana", Some(&token)).send().await.unwrap();
    assert_eq!(r.status(), 200);
}

#[tokio::test]
async fn closed_signup_refuses_strangers_and_makes_no_account() {
    let (app, mock) = setup_closed(None, &["studio.example"]).await;
    let c = browser();
    assert_eq!(sign_in(&app, &mock, &c, good_claims("s1", "ana@gmail.com")).await, "/login?error=signup_closed");
    let users: i64 = sqlx::query_scalar("SELECT count(*) FROM users").fetch_one(&app.db).await.unwrap();
    assert_eq!(users, 0);
}

#[tokio::test]
async fn closed_signup_admits_the_studio_domain() {
    let (app, mock) = setup_closed(None, &["studio.example"]).await;
    let c = browser();
    assert_eq!(sign_in(&app, &mock, &c, good_claims("s1", "Ana@Studio.example")).await, "/");
    assert_eq!(me(&app, &c).await["email"], "ana@studio.example");
}

#[tokio::test]
async fn closed_signup_admits_the_site_admin() {
    let (app, mock) = setup_closed(Some("boss@gmail.com"), &[]).await;
    let c = browser();
    assert_eq!(sign_in(&app, &mock, &c, good_claims("s1", "boss@gmail.com")).await, "/");
}

#[tokio::test]
async fn closed_signup_lets_existing_accounts_in() {
    let (app, mock) = setup_closed(None, &[]).await;
    // Made by an admin outside Google (the dev-user command): it links on first sign-in.
    app.user("ana").await;
    let c = browser();
    assert_eq!(sign_in(&app, &mock, &c, good_claims("s1", "ana@example.com")).await, "/");
    // And signs in again by its Google identity.
    let again = browser();
    assert_eq!(sign_in(&app, &mock, &again, good_claims("s1", "ana@example.com")).await, "/");
}
