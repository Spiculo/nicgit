// SPDX-License-Identifier: AGPL-3.0-or-later
mod common;
use common::git::{git_ok, kimbim_repo};
use serde_json::{json, Value};
use std::io::Read;

async fn setup() -> (common::TestApp, reqwest::Client, reqwest::Client) {
    let app = common::TestApp::spawn().await;
    let alice = app.user("alice").await;
    let carol = app.user("carol").await;
    app.project(&alice, "alice", "house", "private").await;
    alice.put(app.url("/api/projects/alice/house/members/carol")).json(&json!({"role": "reviewer"})).send().await.unwrap();
    (app, alice, carol)
}

fn u(app: &common::TestApp, p: &str) -> String {
    app.url(&format!("/api/projects/alice/house{p}"))
}

const VIEWPOINT: &str = r#"{"camera":{"position":[10,8,12],"direction":[-0.6,-0.4,-0.7],"up":[0,1,0],"fov":45},"selected":["0Zukam4n1BpBYDaJ2UpTma"]}"#;

#[tokio::test]
async fn issues_crud_and_isolation() {
    let (app, alice, carol) = setup().await;
    let r = carol
        .post(u(&app, "/issues"))
        .json(&json!({"title": "Fire rating missing", "body": "W-1 has no rating", "priority": "high",
                       "elements": ["0Zukam4n1BpBYDaJ2UpTma"], "viewpoint": serde_json::from_str::<Value>(VIEWPOINT).unwrap(),
                       "assignee": "alice"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200, "{}", r.text().await.unwrap());
    let list: Value = alice.get(u(&app, "/issues?state=open")).send().await.unwrap().json().await.unwrap();
    assert_eq!(list[0]["number"], 1);
    assert_eq!(list[0]["assignee"], "alice");
    let r = alice.post(u(&app, "/issues/1/comments")).json(&json!({"body": "Will fix"})).send().await.unwrap();
    assert_eq!(r.status(), 200);
    let r = alice.patch(u(&app, "/issues/1")).json(&json!({"state": "closed"})).send().await.unwrap();
    assert_eq!(r.status(), 200);
    let one: Value = carol.get(u(&app, "/issues/1")).send().await.unwrap().json().await.unwrap();
    assert_eq!(one["issue"]["state"], "closed");
    assert_eq!(one["comments"][0]["body"], "Will fix");
    assert_eq!(one["issue"]["viewpoint"]["selected"][0], "0Zukam4n1BpBYDaJ2UpTma");
    let mallory = app.user("mallory").await;
    assert_eq!(mallory.get(u(&app, "/issues/1")).send().await.unwrap().status(), 404);
    // Assignment notifies.
    let inbox: Value = alice.get(app.url("/api/me/inbox")).send().await.unwrap().json().await.unwrap();
    assert!(inbox["notifications"].as_array().unwrap().iter().any(|n| n["kind"] == "issue.assigned"), "{inbox}");
}

#[tokio::test]
async fn bcf_export_then_import() {
    let (app, alice, _carol) = setup().await;
    alice
        .post(u(&app, "/issues"))
        .json(&json!({"title": "Door clash <A&B>", "body": "check", "viewpoint": serde_json::from_str::<Value>(VIEWPOINT).unwrap()}))
        .send()
        .await
        .unwrap();
    let r = alice.get(u(&app, "/issues/export.bcfzip")).send().await.unwrap();
    assert_eq!(r.status(), 200);
    let bytes = r.bytes().await.unwrap();
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes.to_vec())).unwrap();
    let names: Vec<String> = (0..zip.len()).map(|i| zip.by_index(i).unwrap().name().to_string()).collect();
    assert!(names.contains(&"bcf.version".to_string()), "{names:?}");
    let markup = names.iter().find(|n| n.ends_with("/markup.bcf")).unwrap().clone();
    let mut xml = String::new();
    zip.by_name(&markup).unwrap().read_to_string(&mut xml).unwrap();
    assert!(xml.contains("<Title>Door clash &lt;A&amp;B&gt;</Title>"), "{xml}");
    assert!(names.iter().any(|n| n.ends_with(".bcfv")));

    // Importing the same file into another project recreates the topic with its viewpoint.
    app.project(&alice, "alice", "other", "private").await;
    let part = reqwest::multipart::Part::bytes(bytes.to_vec()).file_name("issues.bcfzip");
    let r = alice
        .post(app.url("/api/projects/alice/other/issues/import"))
        .multipart(reqwest::multipart::Form::new().part("file", part))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200, "{}", r.text().await.unwrap());
    let one: Value = alice.get(app.url("/api/projects/alice/other/issues/1")).send().await.unwrap().json().await.unwrap();
    assert_eq!(one["issue"]["title"], "Door clash <A&B>");
    assert_eq!(one["issue"]["viewpoint"]["selected"][0], "0Zukam4n1BpBYDaJ2UpTma");
}

#[tokio::test]
async fn milestone_creates_tag() {
    let (app, alice, carol) = setup().await;
    let at = app.token(&alice).await;
    let src = kimbim_repo().await;
    let url = format!("http://git:{at}@{}/alice/house.git", app.base.trim_start_matches("http://"));
    git_ok(src.path(), &["push", &url, "main"]).await;
    let body = json!({"phase": "permit", "tag": "permit-set-1", "rev": "main", "title": "Permit set", "notes": "Submitted to the city"});
    assert_eq!(carol.post(u(&app, "/milestones")).json(&body).send().await.unwrap().status(), 403);
    let r = alice.post(u(&app, "/milestones")).json(&body).send().await.unwrap();
    assert_eq!(r.status(), 200, "{}", r.text().await.unwrap());
    let tags = git_ok(src.path(), &["ls-remote", "--tags", &url]).await;
    assert!(tags.contains("refs/tags/permit-set-1"), "{tags}");
    let list: Value = carol.get(u(&app, "/milestones")).send().await.unwrap().json().await.unwrap();
    assert_eq!(list[0]["phase"], "permit");
    let id = list[0]["id"].as_str().unwrap();
    assert_eq!(alice.post(u(&app, &format!("/milestones/{id}/signoff"))).send().await.unwrap().status(), 200);
    let list: Value = carol.get(u(&app, "/milestones")).send().await.unwrap().json().await.unwrap();
    assert_eq!(list[0]["signed_off_by"], "alice");
    // Bad phases and duplicate names are refused.
    let r = alice.post(u(&app, "/milestones")).json(&json!({"phase": "party", "tag": "x", "rev": "main", "title": "x"})).send().await.unwrap();
    assert_eq!(r.status(), 400);
    let r = alice.post(u(&app, "/milestones")).json(&body).send().await.unwrap();
    assert_eq!(r.status(), 409);
}

#[tokio::test]
async fn import_upload_creates_project_and_job() {
    let app = common::TestApp::spawn().await;
    let alice = app.user("alice").await;
    let part = reqwest::multipart::Part::bytes(b"PK\x05\x06fake".to_vec()).file_name("tower.kim");
    let form = reqwest::multipart::Form::new().text("owner", "alice").text("slug", "tower").part("file", part);
    let r = alice.post(app.url("/api/projects/import")).multipart(form).send().await.unwrap();
    assert_eq!(r.status(), 200, "{}", r.text().await.unwrap());
    let v: Value = r.json().await.unwrap();
    assert_eq!(v["project"]["full_name"], "alice/tower");
    let job: Value = alice.get(app.url(&format!("/api/jobs/{}", v["job"].as_str().unwrap()))).send().await.unwrap().json().await.unwrap();
    assert_eq!(job["kind"], "import_kim");
    // Only .kim and .ifc are accepted, and size is limited.
    let part = reqwest::multipart::Part::bytes(b"x".to_vec()).file_name("model.rvt");
    let form = reqwest::multipart::Form::new().text("owner", "alice").text("slug", "rvt").part("file", part);
    assert_eq!(alice.post(app.url("/api/projects/import")).multipart(form).send().await.unwrap().status(), 400);
    let big = vec![0u8; (app.cfg.max_file_mb as usize + 1) << 20];
    let part = reqwest::multipart::Part::bytes(big).file_name("big.ifc");
    let form = reqwest::multipart::Form::new().text("owner", "alice").text("slug", "big").part("file", part);
    assert_eq!(alice.post(app.url("/api/projects/import")).multipart(form).send().await.unwrap().status(), 413);
    // A failed upload leaves no half-made project behind.
    assert_eq!(alice.get(app.url("/api/projects/alice/big")).send().await.unwrap().status(), 404);
}

#[tokio::test]
async fn export_kim_is_a_job() {
    let (app, alice, _) = setup().await;
    let r = alice.get(u(&app, "/export.kim?scope=latest")).send().await.unwrap();
    assert_eq!(r.status(), 202);
    let r = alice.get(u(&app, "/export.kim?scope=everything")).send().await.unwrap();
    assert_eq!(r.status(), 400);
}
