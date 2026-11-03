// SPDX-License-Identifier: AGPL-3.0-or-later
//! Job-backed views: compare, settings diff, element history and the IFC of a version.
mod common;
use common::engine;
use serde_json::Value;

#[tokio::test]
async fn compare_is_queued_then_cached() {
    let app = common::TestApp::spawn().await;
    let alice = app.user("alice").await;
    let at = app.token(&alice).await;
    app.project(&alice, "alice", "house", "private").await;
    let src = common::git::kimbim_repo().await;
    common::git::write_record(src.path(), "objects/cd/1111111111111111111111.json", "{\"a\":{\"Name\":\"W-2\"},\"t\":\"IfcWall\"}");
    common::git::git_ok(src.path(), &["add", "-A"]).await;
    common::git::git_ok(src.path(), &["commit", "-qm", "two"]).await;
    let url = format!("http://git:{at}@{}/alice/house.git", app.base.trim_start_matches("http://"));
    common::git::git_ok(src.path(), &["push", &url, "main"]).await;
    let r = alice.get(app.url("/api/projects/alice/house/compare?from=main~1&to=main")).send().await.unwrap();
    assert_eq!(r.status(), 202);
    let first: Value = r.json().await.unwrap();
    let again: Value = alice.get(app.url("/api/projects/alice/house/compare?from=main~1&to=main")).send().await.unwrap().json().await.unwrap();
    assert_eq!(first["job"], again["job"], "same pair, same job");
    let r = alice.get(app.url("/api/projects/alice/house/compare?from=--x&to=main")).send().await.unwrap();
    assert_eq!(r.status(), 400);
    for path in ["/versions/main/model.ifc", "/elements/0Zukam4n1BpBYDaJ2UpTma/history", "/settings-diff?from=main~1&to=main"] {
        let r = alice.get(app.url(&format!("/api/projects/alice/house{path}"))).send().await.unwrap();
        assert_eq!(r.status(), 202, "{path}");
    }
    let mallory = app.user("mallory").await;
    let r = mallory.get(app.url("/api/projects/alice/house/compare?from=main~1&to=main")).send().await.unwrap();
    assert_eq!(r.status(), 404);
}

#[tokio::test]
async fn compare_and_model_with_engine() {
    if !engine::enabled() {
        return eprintln!("skipped: set NICGIT_TEST_ENGINE=1");
    }
    let app = common::TestApp::spawn().await;
    let alice = engine::push_m5(&app).await;
    let q = "/api/projects/alice/m5/compare?from=main~1&to=Scheme%2520B";
    assert_eq!(alice.get(app.url(q)).send().await.unwrap().status(), 202);
    let ifc = "/api/projects/alice/m5/versions/main/model.ifc";
    assert_eq!(alice.get(app.url(ifc)).send().await.unwrap().status(), 202);
    engine::drain(&app).await;
    let v: Value = alice.get(app.url(q)).send().await.unwrap().json().await.unwrap();
    assert!(v["changes"]["modified"].as_array().unwrap().len() >= 1, "{v}");
    let r = alice.get(app.url(ifc)).send().await.unwrap();
    assert_eq!(r.status(), 200);
    assert_eq!(r.headers()["content-type"], "application/x-step");
    assert!(r.text().await.unwrap().starts_with("ISO-10303-21"));
}

#[tokio::test]
async fn anonymous_cannot_queue_engine_work_on_public_projects() {
    let app = common::TestApp::spawn().await;
    let alice = app.user("alice").await;
    let at = app.token(&alice).await;
    app.project(&alice, "alice", "open", "public").await;
    let src = common::git::kimbim_repo().await;
    common::git::write_record(src.path(), "objects/cd/1111111111111111111111.json", "{\"a\":{\"Name\":\"W-2\"},\"t\":\"IfcWall\"}");
    common::git::git_ok(src.path(), &["add", "-A"]).await;
    common::git::git_ok(src.path(), &["commit", "-qm", "two"]).await;
    let url = format!("http://git:{at}@{}/alice/open.git", app.base.trim_start_matches("http://"));
    common::git::git_ok(src.path(), &["push", &url, "main"]).await;
    let anon = reqwest::Client::new();
    for path in ["/compare?from=main~1&to=main", "/elements/0Zukam4n1BpBYDaJ2UpTma/history", "/settings-diff?from=main~1&to=main"] {
        let r = anon.get(app.url(&format!("/api/projects/alice/open{path}"))).send().await.unwrap();
        assert_eq!(r.status(), 401, "{path}");
    }
    // Once a signed-in user asked, the same answer is shared (here: still being computed).
    assert_eq!(alice.get(app.url("/api/projects/alice/open/compare?from=main~1&to=main")).send().await.unwrap().status(), 202);
    assert_eq!(anon.get(app.url("/api/projects/alice/open/compare?from=main~1&to=main")).send().await.unwrap().status(), 202);
    // Viewing the model stays open to everyone.
    assert_eq!(anon.get(app.url("/api/projects/alice/open/versions/main/model.ifc")).send().await.unwrap().status(), 202);
}
