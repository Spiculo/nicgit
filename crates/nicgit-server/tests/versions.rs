// SPDX-License-Identifier: AGPL-3.0-or-later
mod common;
use common::git::{git_ok, kimbim_repo, write_record};
use serde_json::Value;

async fn pushed(app: &common::TestApp) -> (reqwest::Client, tempfile::TempDir) {
    let alice = app.user("alice").await;
    let at = app.token(&alice).await;
    app.project(&alice, "alice", "house", "private").await;
    let src = kimbim_repo().await;
    write_record(src.path(), "objects/cd/1111111111111111111111.json", "{\"a\":{\"Name\":\"W-2\"},\"t\":\"IfcWall\"}");
    git_ok(src.path(), &["add", "-A"]).await;
    git_ok(src.path(), &["commit", "-qm", "Second wall"]).await;
    git_ok(src.path(), &["branch", "agent/widen-windows"]).await;
    let url = format!("http://git:{at}@{}/alice/house.git", app.base.trim_start_matches("http://"));
    git_ok(src.path(), &["push", &url, "main", "agent/widen-windows"]).await;
    (alice, src)
}

#[tokio::test]
async fn empty_project_overview() {
    let app = common::TestApp::spawn().await;
    let alice = app.user("alice").await;
    app.project(&alice, "alice", "house", "private").await;
    let r = alice.get(app.url("/api/projects/alice/house/overview")).send().await.unwrap();
    assert_eq!(r.status(), 200);
    let v: Value = r.json().await.unwrap();
    assert_eq!(v["empty"], true);
    for route in ["/versions", "/options", "/tags", "/files"] {
        let r = alice.get(app.url(&format!("/api/projects/alice/house{route}"))).send().await.unwrap();
        assert_eq!(r.status(), 200, "{route}");
    }
}

#[tokio::test]
async fn versions_list_after_push() {
    let app = common::TestApp::spawn().await;
    let (alice, _src) = pushed(&app).await;
    let v: Value = alice.get(app.url("/api/projects/alice/house/versions?option=main")).send().await.unwrap().json().await.unwrap();
    let list = v.as_array().unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0]["message"], "Second wall");
    assert_eq!(list[1]["message"], "The first version");
    assert_eq!(list[0]["parents"][0], list[1]["id"]);
    let one: Value = alice
        .get(app.url(&format!("/api/projects/alice/house/versions/{}", list[0]["id"].as_str().unwrap())))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(one["counts"]["objects"], 2);
    assert_eq!(one["schema"], "IFC4");
    assert_eq!(one["settings"]["units"], "mm");
    let overview: Value = alice.get(app.url("/api/projects/alice/house/overview")).send().await.unwrap().json().await.unwrap();
    assert_eq!(overview["empty"], false);
    assert_eq!(overview["latest"]["message"], "Second wall");
}

#[tokio::test]
async fn options_flag_agent_prefix() {
    let app = common::TestApp::spawn().await;
    let (alice, _src) = pushed(&app).await;
    let v: Value = alice.get(app.url("/api/projects/alice/house/options")).send().await.unwrap().json().await.unwrap();
    let opts = v.as_array().unwrap();
    let agent = opts.iter().find(|o| o["name"] == "agent/widen-windows").unwrap();
    assert_eq!(agent["is_agent"], true);
    let main = opts.iter().find(|o| o["name"] == "main").unwrap();
    assert_eq!(main["is_default"], true);
    assert_eq!(main["is_agent"], false);
}

#[tokio::test]
async fn blob_of_record_is_json() {
    let app = common::TestApp::spawn().await;
    let (alice, _src) = pushed(&app).await;
    let v: Value = alice
        .get(app.url("/api/projects/alice/house/blob?rev=main&path=objects/cd/1111111111111111111111.json"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(v["a"]["Name"], "W-2");
    let files: Value = alice.get(app.url("/api/projects/alice/house/files?rev=main")).send().await.unwrap().json().await.unwrap();
    let names: Vec<&str> = files["entries"].as_array().unwrap().iter().map(|e| e["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"objects") && names.contains(&"kimbim.json"));
    // Option-like revisions are refused, never passed to git as flags.
    let r = alice.get(app.url("/api/projects/alice/house/blob?rev=--output=x&path=kimbim.json")).send().await.unwrap();
    assert_eq!(r.status(), 400);
    let r = alice.get(app.url("/api/projects/alice/house/blob?rev=main&path=../../etc/passwd")).send().await.unwrap();
    assert_eq!(r.status(), 404);
}
