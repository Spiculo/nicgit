// SPDX-License-Identifier: AGPL-3.0-or-later
mod common;
use serde_json::{json, Value};

#[tokio::test]
async fn signup_login_me() {
    let app = common::TestApp::spawn().await;
    let alice = app.user("alice").await;
    let me: Value = alice.get(app.url("/api/me")).send().await.unwrap().json().await.unwrap();
    assert_eq!(me["slug"], "alice");
    assert!(me.get("password_hash").is_none());

    let fresh = reqwest::Client::builder().cookie_store(true).build().unwrap();
    assert_eq!(fresh.get(app.url("/api/me")).send().await.unwrap().status(), 401);
    let r = fresh
        .post(app.url("/api/auth/login"))
        .json(&json!({"login": "alice@example.com", "password": "correct horse battery"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let me: Value = fresh.get(app.url("/api/me")).send().await.unwrap().json().await.unwrap();
    assert_eq!(me["slug"], "alice");

    fresh.post(app.url("/api/auth/logout")).send().await.unwrap();
    assert_eq!(fresh.get(app.url("/api/me")).send().await.unwrap().status(), 401);
}

#[tokio::test]
async fn signup_rejects_bad_input() {
    let app = common::TestApp::spawn().await;
    let c = reqwest::Client::new();
    let bad = [
        json!({"slug": "api", "email": "a@b.c", "password": "correct horse battery"}),
        json!({"slug": "bob", "email": "nope", "password": "correct horse battery"}),
        json!({"slug": "bob", "email": "a@b.c", "password": "short"}),
    ];
    for body in bad {
        let r = c.post(app.url("/api/auth/signup")).json(&body).send().await.unwrap();
        assert_eq!(r.status(), 400, "{body}");
    }
    app.user("bob").await;
    let r = c
        .post(app.url("/api/auth/signup"))
        .json(&json!({"slug": "bob", "email": "other@example.com", "password": "correct horse battery"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 409);
}

#[tokio::test]
async fn wrong_password_401() {
    let app = common::TestApp::spawn().await;
    app.user("alice").await;
    let r = reqwest::Client::new()
        .post(app.url("/api/auth/login"))
        .json(&json!({"login": "alice", "password": "wrong password!!"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 401);
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
    app.user("carol").await;
    alice.post(app.url("/api/orgs")).json(&json!({"slug": "studio"})).send().await.unwrap();
    alice.put(app.url("/api/orgs/studio/members/bob")).json(&json!({"role": "member"})).send().await.unwrap();
    let r = bob.put(app.url("/api/orgs/studio/members/carol")).json(&json!({"role": "member"})).send().await.unwrap();
    assert_eq!(r.status(), 403);
    // A stranger does not even learn the members.
    let carol = reqwest::Client::builder().cookie_store(true).build().unwrap();
    carol
        .post(app.url("/api/auth/login"))
        .json(&json!({"login": "carol", "password": "correct horse battery"}))
        .send()
        .await
        .unwrap();
    let org: Value = carol.get(app.url("/api/orgs/studio")).send().await.unwrap().json().await.unwrap();
    assert!(org.get("members").is_none() || org["members"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn signup_writes_audit() {
    let app = common::TestApp::spawn().await;
    app.user("alice").await;
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM audit_log WHERE action = 'user.signup'")
        .fetch_one(&app.db)
        .await
        .unwrap();
    assert_eq!(n, 1);
}
