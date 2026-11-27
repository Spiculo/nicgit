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

#[tokio::test]
async fn account_projects_show_public_and_accessible_projects() {
    let app = common::TestApp::spawn().await;
    let alice = app.user("alice").await;
    let bob = app.user("bob").await;
    app.project(&alice, "alice", "private-house", "private").await;
    app.project(&alice, "alice", "open-house", "public").await;

    let anon = reqwest::Client::new();
    let public: Value = anon.get(app.url("/api/accounts/alice/projects")).send().await.unwrap().json().await.unwrap();
    assert_eq!(public.as_array().unwrap().len(), 1);
    assert_eq!(public[0]["slug"], "open-house");

    let bob_sees: Value = bob.get(app.url("/api/accounts/alice/projects")).send().await.unwrap().json().await.unwrap();
    assert_eq!(bob_sees.as_array().unwrap().len(), 1, "a private project is absent without a grant");

    let alice_sees: Value = alice.get(app.url("/api/accounts/alice/projects")).send().await.unwrap().json().await.unwrap();
    assert_eq!(alice_sees.as_array().unwrap().len(), 2);

    alice.put(app.url("/api/projects/alice/private-house/members/bob")).json(&json!({"role": "viewer"})).send().await.unwrap();
    let bob_sees: Value = bob.get(app.url("/api/accounts/alice/projects")).send().await.unwrap().json().await.unwrap();
    assert_eq!(bob_sees.as_array().unwrap().len(), 2);
    let private = bob_sees.as_array().unwrap().iter().find(|p| p["slug"] == "private-house").unwrap();
    assert_eq!(private["role"], "viewer");
}

#[tokio::test]
async fn home_orders_repositories_by_activity_and_returns_sections() {
    let app = common::TestApp::spawn().await;
    let alice = app.user("alice").await;
    let old = app.project(&alice, "alice", "old-house", "private").await;
    let new = app.project(&alice, "alice", "new-house", "private").await;
    let old_id = old["id"].as_str().unwrap();
    let new_id = new["id"].as_str().unwrap();
    let alice_id: uuid::Uuid = sqlx::query_scalar("SELECT id FROM accounts WHERE slug = 'alice'").fetch_one(&app.db).await.unwrap();

    sqlx::query("UPDATE activity SET created_at = now() - interval '3 days' WHERE project_id = $1")
        .bind(uuid::Uuid::parse_str(old_id).unwrap())
        .execute(&app.db)
        .await
        .unwrap();
    sqlx::query("UPDATE activity SET created_at = now() - interval '1 hour' WHERE project_id = $1")
        .bind(uuid::Uuid::parse_str(new_id).unwrap())
        .execute(&app.db)
        .await
        .unwrap();
    sqlx::query("INSERT INTO milestones (id, project_id, phase, tag, commit_id, title, created_by) VALUES ($1, $2, 'concept', 'v1', 'abc1234', 'First release', $3)")
        .bind(uuid::Uuid::new_v4())
        .bind(uuid::Uuid::parse_str(new_id).unwrap())
        .bind(alice_id)
        .execute(&app.db)
        .await
        .unwrap();
    sqlx::query("INSERT INTO proposals (id, project_id, number, title, source, target, author_id, source_head, target_head) VALUES ($1, $2, 1, 'Glaze the wall', 'Scheme B', 'main', $3, 'abc1234', 'def5678')")
        .bind(uuid::Uuid::new_v4())
        .bind(uuid::Uuid::parse_str(new_id).unwrap())
        .bind(alice_id)
        .execute(&app.db)
        .await
        .unwrap();
    sqlx::query("INSERT INTO issues (id, project_id, number, title, author_id) VALUES ($1, $2, 1, 'Check the windows', $3)")
        .bind(uuid::Uuid::new_v4())
        .bind(uuid::Uuid::parse_str(new_id).unwrap())
        .bind(alice_id)
        .execute(&app.db)
        .await
        .unwrap();
    sqlx::query("INSERT INTO jobs (id, project_id, kind, status, result, finished_at) VALUES ($1, $2, 'checks', 'done', $3, now())")
        .bind(uuid::Uuid::new_v4())
        .bind(uuid::Uuid::parse_str(new_id).unwrap())
        .bind(json!({"passed": true}))
        .execute(&app.db)
        .await
        .unwrap();
    sqlx::query("INSERT INTO notifications (id, user_id, kind, data) VALUES ($1, $2, 'proposal.review_requested', $3)")
        .bind(uuid::Uuid::new_v4())
        .bind(alice_id)
        .bind(json!({"project": "alice/new-house", "number": 1, "title": "Glaze the wall"}))
        .execute(&app.db)
        .await
        .unwrap();

    let home: Value = alice.get(app.url("/api/me/home")).send().await.unwrap().json().await.unwrap();
    assert_eq!(home["repos"][0]["slug"], "new-house");
    assert_eq!(home["repos"][1]["slug"], "old-house");
    assert_eq!(home["repos"][0]["open_proposals"], 1);
    assert_eq!(home["repos"][0]["open_issues"], 1);
    assert_eq!(home["releases"][0]["tag"], "v1");
    assert_eq!(home["proposals"][0]["project"], "alice/new-house");
    assert_eq!(home["issues"][0]["number"], 1);
    assert_eq!(home["runs"][0]["status"], "done");
    assert_eq!(home["posts"].as_array().unwrap().len(), 1);
    assert_eq!(home["packages"].as_array().unwrap().len(), 0);
    assert!(home["changes"].is_null());
    assert_eq!(reqwest::Client::new().get(app.url("/api/me/home")).send().await.unwrap().status(), 401);
}
