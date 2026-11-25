// SPDX-License-Identifier: AGPL-3.0-or-later
//! Git smart HTTP against a real `git` client.
mod common;
use common::git::{git, git_ok, kimbim_repo, write_record};
use serde_json::{json, Value};

fn remote(app: &common::TestApp, token: Option<&str>, path: &str) -> String {
    let host = app.base.trim_start_matches("http://");
    match token {
        Some(t) => format!("http://git:{t}@{host}/{path}.git"),
        None => format!("http://{host}/{path}.git"),
    }
}

#[tokio::test]
async fn clone_empty_project() {
    let app = common::TestApp::spawn().await;
    let alice = app.user("alice").await;
    let token = app.token(&alice).await;
    app.project(&alice, "alice", "house", "private").await;
    let dir = tempfile::tempdir().unwrap();
    let out = git(dir.path(), &["clone", &remote(&app, Some(&token), "alice/house"), "c"]).await;
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
}

#[tokio::test]
async fn push_then_clone_same_tree_hash() {
    let app = common::TestApp::spawn().await;
    let alice = app.user("alice").await;
    let token = app.token(&alice).await;
    app.project(&alice, "alice", "house", "private").await;
    let src = kimbim_repo().await;
    git_ok(src.path(), &["push", &remote(&app, Some(&token), "alice/house"), "main"]).await;
    let tree = git_ok(src.path(), &["rev-parse", "main^{tree}"]).await;
    let dst = tempfile::tempdir().unwrap();
    git_ok(dst.path(), &["clone", "--bare", &remote(&app, Some(&token), "alice/house"), "c.git"]).await;
    let cloned = git_ok(&dst.path().join("c.git"), &["rev-parse", "main^{tree}"]).await;
    assert_eq!(tree, cloned);
}

#[tokio::test]
async fn push_requires_auth_private() {
    let app = common::TestApp::spawn().await;
    let alice = app.user("alice").await;
    app.project(&alice, "alice", "house", "private").await;
    let src = kimbim_repo().await;
    let out = git(src.path(), &["push", &remote(&app, None, "alice/house"), "main"]).await;
    assert!(!out.status.success());
    let out = git(src.path(), &["ls-remote", &remote(&app, None, "alice/house")]).await;
    assert!(!out.status.success());
    // An authenticated stranger is told the project does not exist.
    let mallory = app.user("mallory").await;
    let mt = app.token(&mallory).await;
    let r = reqwest::Client::new()
        .get(app.url("/alice/house.git/info/refs?service=git-upload-pack"))
        .basic_auth("git", Some(&mt))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 404);
    let r = reqwest::Client::new().get(app.url("/alice/house.git/info/refs?service=git-upload-pack")).send().await.unwrap();
    assert_eq!(r.status(), 401);
    assert!(r.headers().get("www-authenticate").is_some());
}

#[tokio::test]
async fn anonymous_clone_public() {
    let app = common::TestApp::spawn().await;
    let alice = app.user("alice").await;
    let token = app.token(&alice).await;
    app.project(&alice, "alice", "open", "public").await;
    let src = kimbim_repo().await;
    git_ok(src.path(), &["push", &remote(&app, Some(&token), "alice/open"), "main"]).await;
    let dst = tempfile::tempdir().unwrap();
    git_ok(dst.path(), &["clone", &remote(&app, None, "alice/open"), "c"]).await;
    // ...but cannot push.
    let out = git(src.path(), &["push", &remote(&app, None, "alice/open"), "main:other"]).await;
    assert!(!out.status.success());
}

#[tokio::test]
async fn push_rejects_invalid_record() {
    let app = common::TestApp::spawn().await;
    let alice = app.user("alice").await;
    let token = app.token(&alice).await;
    app.project(&alice, "alice", "house", "private").await;
    let src = kimbim_repo().await;
    write_record(src.path(), "objects/ab/2O2Fr$t4X7Zf8NOew3FLOH.json", "<<<<<<< HEAD\n{\"a\":1}\n=======\n");
    git_ok(src.path(), &["add", "-A"]).await;
    git_ok(src.path(), &["commit", "-qm", "text merge leftovers"]).await;
    let out = git(src.path(), &["push", &remote(&app, Some(&token), "alice/house"), "main"]).await;
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("NicGit:"), "{err}");
    assert!(err.contains("objects/ab/2O2Fr$t4X7Zf8NOew3FLOH.json"), "{err}");
}

#[tokio::test]
async fn reviewer_cannot_push() {
    let app = common::TestApp::spawn().await;
    let alice = app.user("alice").await;
    let bob = app.user("bob").await;
    let bt = app.token(&bob).await;
    app.project(&alice, "alice", "house", "private").await;
    alice.put(app.url("/api/projects/alice/house/members/bob")).json(&json!({"role": "reviewer"})).send().await.unwrap();
    let src = kimbim_repo().await;
    let dst = tempfile::tempdir().unwrap();
    git_ok(dst.path(), &["clone", &remote(&app, Some(&bt), "alice/house"), "c"]).await;
    let out = git(src.path(), &["push", &remote(&app, Some(&bt), "alice/house"), "main"]).await;
    assert!(!out.status.success());
}

#[tokio::test]
async fn protected_option_requires_lead() {
    let app = common::TestApp::spawn().await;
    let alice = app.user("alice").await;
    let at = app.token(&alice).await;
    let bob = app.user("bob").await;
    let bt = app.token(&bob).await;
    let p = app.project(&alice, "alice", "house", "private").await;
    alice.put(app.url("/api/projects/alice/house/members/bob")).json(&json!({"role": "member"})).send().await.unwrap();
    let src = kimbim_repo().await;
    git_ok(src.path(), &["push", &remote(&app, Some(&at), "alice/house"), "main"]).await;
    sqlx::query("INSERT INTO option_rules (project_id, option) VALUES ($1, 'main')")
        .bind(uuid::Uuid::parse_str(p["id"].as_str().unwrap()).unwrap())
        .execute(&app.db)
        .await
        .unwrap();
    write_record(src.path(), "objects/cd/1111111111111111111111.json", "{\"a\":{\"Name\":\"W-2\"},\"t\":\"IfcWall\"}");
    git_ok(src.path(), &["add", "-A"]).await;
    git_ok(src.path(), &["commit", "-qm", "second wall"]).await;
    let out = git(src.path(), &["push", &remote(&app, Some(&bt), "alice/house"), "main"]).await;
    assert!(!out.status.success(), "a member may not push to a protected option");
    assert!(String::from_utf8_lossy(&out.stderr).contains("protected"));
    // Bob may push the same versions to his own option.
    git_ok(src.path(), &["push", &remote(&app, Some(&bt), "alice/house"), "main:bob-scheme"]).await;
    // The lead/admin may fast-forward main, but never rewrite it.
    git_ok(src.path(), &["push", &remote(&app, Some(&at), "alice/house"), "main"]).await;
    git_ok(src.path(), &["reset", "-q", "--hard", "HEAD~1"]).await;
    let out = git(src.path(), &["push", "--force", &remote(&app, Some(&at), "alice/house"), "main"]).await;
    assert!(!out.status.success(), "protected options are never rewritten");
}

#[tokio::test]
async fn reserved_refs_rejected() {
    let app = common::TestApp::spawn().await;
    let alice = app.user("alice").await;
    let at = app.token(&alice).await;
    app.project(&alice, "alice", "house", "private").await;
    let src = kimbim_repo().await;
    let out = git(src.path(), &["push", &remote(&app, Some(&at), "alice/house"), "main:refs/proposals/1/head"]).await;
    assert!(!out.status.success());
}

#[tokio::test]
async fn push_records_activity() {
    let app = common::TestApp::spawn().await;
    let alice = app.user("alice").await;
    let at = app.token(&alice).await;
    app.project(&alice, "alice", "house", "private").await;
    let src = kimbim_repo().await;
    git_ok(src.path(), &["push", &remote(&app, Some(&at), "alice/house"), "main"]).await;
    for _ in 0..50 {
        let feed: Value = alice.get(app.url("/api/projects/alice/house/activity")).send().await.unwrap().json().await.unwrap();
        if let Some(push) = feed.as_array().unwrap().iter().find(|a| a["kind"] == "push") {
            assert_eq!(push["actor"], "alice");
            assert_eq!(push["data"]["updates"][0]["name"], "refs/heads/main");
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    panic!("no push activity recorded");
}

/// The real NicBim M5 project (two design options combined) survives a push and clone unchanged.
#[tokio::test]
async fn golden_m5_push_roundtrip() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../nicbim/build/m5/project/.git");
    if !fixture.is_dir() {
        eprintln!("skipped: {} missing (run `pixi run m5` in nicbim)", fixture.display());
        return;
    }
    let app = common::TestApp::spawn().await;
    let alice = app.user("alice").await;
    let at = app.token(&alice).await;
    app.project(&alice, "alice", "m5", "private").await;
    let fx = fixture.to_string_lossy().to_string();
    let here = std::path::Path::new(".");
    git_ok(here, &["--git-dir", &fx, "push", &remote(&app, Some(&at), "alice/m5"), "refs/heads/*:refs/heads/*"]).await;
    let dst = tempfile::tempdir().unwrap();
    git_ok(dst.path(), &["clone", "--bare", &remote(&app, Some(&at), "alice/m5"), "c.git"]).await;
    let c = dst.path().join("c.git");
    for branch in ["main", "Scheme%20B"] {
        let want = git_ok(here, &["--git-dir", &fx, "rev-parse", &format!("refs/heads/{branch}")]).await;
        let got = git_ok(&c, &["rev-parse", &format!("refs/heads/{branch}")]).await;
        assert_eq!(want, got, "{branch}");
    }
}
