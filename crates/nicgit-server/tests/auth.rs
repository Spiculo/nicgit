// SPDX-License-Identifier: AGPL-3.0-or-later
mod common;
use serde_json::{json, Value};

#[tokio::test]
async fn session_me_and_logout() {
    let app = common::TestApp::spawn().await;
    let alice = app.user("alice").await;
    let me: Value = alice.get(app.url("/api/me")).send().await.unwrap().json().await.unwrap();
    assert_eq!(me["slug"], "alice");
    assert!(me.get("google_sub").is_none());

    let fresh = reqwest::Client::new();
    assert_eq!(fresh.get(app.url("/api/me")).send().await.unwrap().status(), 401);

    let r = alice.post(app.url("/api/auth/logout")).send().await.unwrap();
    assert_eq!(r.status(), 200);
    let sessions: i64 = sqlx::query_scalar("SELECT count(*) FROM sessions").fetch_one(&app.db).await.unwrap();
    assert_eq!(sessions, 0, "the session is gone on the server, not just the cookie");
}

#[tokio::test]
async fn pat_authenticates_api() {
    let app = common::TestApp::spawn().await;
    let alice = app.user("alice").await;
    let token = app.token(&alice).await;
    assert!(token.starts_with("ngp_"));
    let anon = reqwest::Client::new();
    let me: Value = anon.get(app.url("/api/me")).bearer_auth(&token).send().await.unwrap().json().await.unwrap();
    assert_eq!(me["slug"], "alice");
    let r = anon.get(app.url("/api/me")).basic_auth("whatever", Some(&token)).send().await.unwrap();
    assert_eq!(r.status(), 200);
}

#[tokio::test]
async fn revoked_token_401() {
    let app = common::TestApp::spawn().await;
    let alice = app.user("alice").await;
    let token = app.token(&alice).await;
    let list: Value = alice.get(app.url("/api/me/tokens")).send().await.unwrap().json().await.unwrap();
    let id = list[0]["id"].as_str().unwrap().to_string();
    assert!(list[0].get("token").is_none(), "token secret must never be listed");
    alice.delete(app.url(&format!("/api/me/tokens/{id}"))).send().await.unwrap();
    let r = reqwest::Client::new().get(app.url("/api/me")).bearer_auth(&token).send().await.unwrap();
    assert_eq!(r.status(), 401);
}

#[tokio::test]
async fn org_admin_adds_member() {
    let app = common::TestApp::spawn().await;
    let alice = app.user("alice").await;
    let bob = app.user("bob").await;
    let r = alice.post(app.url("/api/orgs")).json(&json!({"slug": "studio", "display_name": "Studio"})).send().await.unwrap();
    assert_eq!(r.status(), 200);
    let r = alice.put(app.url("/api/orgs/studio/members/bob")).json(&json!({"role": "member"})).send().await.unwrap();
    assert_eq!(r.status(), 200);
    let org: Value = bob.get(app.url("/api/orgs/studio")).send().await.unwrap().json().await.unwrap();
    let members = org["members"].as_array().unwrap();
    assert!(members.iter().any(|m| m["slug"] == "bob" && m["role"] == "member"));
    assert!(members.iter().any(|m| m["slug"] == "alice" && m["role"] == "admin"));
}

#[tokio::test]
async fn non_admin_cannot_add_member() {
    let app = common::TestApp::spawn().await;
    let alice = app.user("alice").await;
    let bob = app.user("bob").await;
    let carol = app.user("carol").await;
    alice.post(app.url("/api/orgs")).json(&json!({"slug": "studio"})).send().await.unwrap();
    alice.put(app.url("/api/orgs/studio/members/bob")).json(&json!({"role": "member"})).send().await.unwrap();
    let r = bob.put(app.url("/api/orgs/studio/members/carol")).json(&json!({"role": "member"})).send().await.unwrap();
    assert_eq!(r.status(), 403);
    // A stranger does not even learn the members.
    let org: Value = carol.get(app.url("/api/orgs/studio")).send().await.unwrap().json().await.unwrap();
    assert!(org.get("members").is_none() || org["members"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn new_user_writes_audit() {
    let app = common::TestApp::spawn().await;
    app.user("alice").await;
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM audit_log WHERE action = 'user.signup'")
        .fetch_one(&app.db)
        .await
        .unwrap();
    assert_eq!(n, 1);
}
