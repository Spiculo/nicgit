// SPDX-License-Identifier: AGPL-3.0-or-later
mod common;
use serde_json::{json, Value};

#[tokio::test]
async fn create_private_project_inits_repo() {
    let app = common::TestApp::spawn().await;
    let alice = app.user("alice").await;
    let p = app.project(&alice, "alice", "house", "private").await;
    let id = p["id"].as_str().unwrap();
    let git = app.data.path().join("repos").join(id).join(".git");
    assert!(git.join("HEAD").is_file());
    assert_eq!(std::fs::read_to_string(git.join("HEAD")).unwrap().trim(), "ref: refs/heads/main");
    assert!(git.join("hooks").join("pre-receive").is_file());

    let got: Value = alice.get(app.url("/api/projects/alice/house")).send().await.unwrap().json().await.unwrap();
    assert_eq!(got["visibility"], "private");
    assert_eq!(got["role"], "admin");

    let list: Value = alice.get(app.url("/api/projects")).send().await.unwrap().json().await.unwrap();
    assert_eq!(list.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn duplicate_or_invalid_project_rejected() {
    let app = common::TestApp::spawn().await;
    let alice = app.user("alice").await;
    app.project(&alice, "alice", "house", "private").await;
    let r = alice.post(app.url("/api/projects")).json(&json!({"owner": "alice", "slug": "house"})).send().await.unwrap();
    assert_eq!(r.status(), 409);
    let r = alice.post(app.url("/api/projects")).json(&json!({"owner": "alice", "slug": "Bad Name"})).send().await.unwrap();
    assert_eq!(r.status(), 400);
    app.user("bob").await;
    let r = alice.post(app.url("/api/projects")).json(&json!({"owner": "bob", "slug": "x"})).send().await.unwrap();
    assert_eq!(r.status(), 403);
}

#[tokio::test]
async fn public_project_readable_anonymously() {
    let app = common::TestApp::spawn().await;
    let alice = app.user("alice").await;
    app.project(&alice, "alice", "open", "public").await;
    let anon = reqwest::Client::new();
    let got: Value = anon.get(app.url("/api/projects/alice/open")).send().await.unwrap().json().await.unwrap();
    assert_eq!(got["role"], "viewer");
    let explore: Value = anon.get(app.url("/api/explore")).send().await.unwrap().json().await.unwrap();
    assert_eq!(explore.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn member_roles_resolve_max() {
    let app = common::TestApp::spawn().await;
    let alice = app.user("alice").await;
    let bob = app.user("bob").await;
    alice.post(app.url("/api/orgs")).json(&json!({"slug": "studio"})).send().await.unwrap();
    alice.put(app.url("/api/orgs/studio/members/bob")).json(&json!({"role": "reviewer"})).send().await.unwrap();
    app.project(&alice, "studio", "tower", "private").await;
    let got: Value = bob.get(app.url("/api/projects/studio/tower")).send().await.unwrap().json().await.unwrap();
    assert_eq!(got["role"], "reviewer");
    // A direct grant above the org role wins.
    let r = alice
        .put(app.url("/api/projects/studio/tower/members/bob"))
        .json(&json!({"role": "lead"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let got: Value = bob.get(app.url("/api/projects/studio/tower")).send().await.unwrap().json().await.unwrap();
    assert_eq!(got["role"], "lead");
    let members: Value = bob.get(app.url("/api/projects/studio/tower/members")).send().await.unwrap().json().await.unwrap();
    assert!(members.as_array().unwrap().iter().any(|m| m["slug"] == "bob" && m["role"] == "lead"));
}

#[tokio::test]
async fn visibility_change_audited() {
    let app = common::TestApp::spawn().await;
    let alice = app.user("alice").await;
    app.project(&alice, "alice", "house", "private").await;
    let r = alice
        .patch(app.url("/api/projects/alice/house"))
        .json(&json!({"visibility": "public", "description": "A small house"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM audit_log WHERE action = 'project.visibility'")
        .fetch_one(&app.db)
        .await
        .unwrap();
    assert_eq!(n, 1);
}

#[tokio::test]
async fn reviewer_cannot_change_settings() {
    let app = common::TestApp::spawn().await;
    let alice = app.user("alice").await;
    let bob = app.user("bob").await;
    app.project(&alice, "alice", "house", "private").await;
    alice.put(app.url("/api/projects/alice/house/members/bob")).json(&json!({"role": "reviewer"})).send().await.unwrap();
    let r = bob.patch(app.url("/api/projects/alice/house")).json(&json!({"visibility": "public"})).send().await.unwrap();
    assert_eq!(r.status(), 403);
    let r = bob.delete(app.url("/api/projects/alice/house")).send().await.unwrap();
    assert_eq!(r.status(), 403);
}

#[tokio::test]
async fn delete_project_removes_repo() {
    let app = common::TestApp::spawn().await;
    let alice = app.user("alice").await;
    let p = app.project(&alice, "alice", "house", "private").await;
    let dir = app.data.path().join("repos").join(p["id"].as_str().unwrap());
    let r = alice.delete(app.url("/api/projects/alice/house")).send().await.unwrap();
    assert_eq!(r.status(), 200);
    assert!(!dir.exists());
    assert_eq!(alice.get(app.url("/api/projects/alice/house")).send().await.unwrap().status(), 404);
}
