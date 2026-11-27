// SPDX-License-Identifier: AGPL-3.0-or-later
//! CI / Actions: the checks runs the web app reads back (not a second execution model).
mod common;
use common::git::{git_ok, kimbim_repo};
use serde_json::{json, Value};

async fn setup() -> (common::TestApp, reqwest::Client, String, String, uuid::Uuid) {
    let app = common::TestApp::spawn().await;
    let alice = app.user("alice").await;
    let at = app.token(&alice).await;
    let p = app.project(&alice, "alice", "house", "private").await;
    let project_id: uuid::Uuid = p["id"].as_str().unwrap().parse().unwrap();
    let r = alice
        .put(app.url("/api/projects/alice/house/rules/main"))
        .json(&json!({"required_approvals": 0, "required_checks": ["schema"]}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let src = kimbim_repo().await;
    let url = format!(
        "http://git:{at}@{}/alice/house.git",
        app.base.trim_start_matches("http://")
    );
    git_ok(src.path(), &["push", &url, "main"]).await;
    let head = git_ok(src.path(), &["rev-parse", "HEAD"]).await;
    (app, alice, head, at, project_id)
}

async fn insert_check_run(app: &common::TestApp, project: uuid::Uuid, head: &str) -> uuid::Uuid {
    let id = uuid::Uuid::new_v4();
    let cache = format!("{project}:checks:{head}:schema");
    sqlx::query(
        "INSERT INTO jobs (id, project_id, kind, args, cache_key, status, progress, result, started_at, finished_at) \
         VALUES ($1, $2, 'checks', $3, $4, 'done', 1, $5, now() - interval '2 seconds', now())",
    )
    .bind(id)
    .bind(project)
    .bind(json!({"rev": head, "checks": ["schema"]}))
    .bind(cache)
    .bind(json!({
        "rev": head,
        "passed": true,
        "seconds": 2.0,
        "checks": [{
            "check": "schema",
            "title": "Schema validation",
            "ok": true,
            "total": 0,
            "counts": {"error": 0, "warning": 0, "info": 0},
            "issues": [],
            "truncated": false,
            "seconds": 2.0,
            "failed": false
        }]
    }))
    .execute(&app.db)
    .await
    .unwrap();
    id
}

#[tokio::test]
async fn runs_details_required_checks_and_report() {
    let (app, alice, head, _at, project) = setup().await;
    let id = insert_check_run(&app, project, &head).await;

    let list: Value = alice
        .get(app.url("/api/projects/alice/house/ci/runs"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let runs = list["runs"].as_array().unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0]["commit"], head);
    assert_eq!(runs[0]["passed"], true);
    assert_eq!(runs[0]["checks"], json!(["schema"]));

    let one: Value = alice
        .get(app.url(&format!("/api/projects/alice/house/ci/runs/{id}")))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(one["run"]["id"], id.to_string());
    assert_eq!(one["run"]["duration"], 2.0);

    let checks: Value = alice
        .get(app.url("/api/projects/alice/house/ci/checks"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let main = checks["options"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["option"] == "main")
        .unwrap()
        .clone();
    assert_eq!(main["required"], json!(["schema"]));
    assert_eq!(main["state"], "passed");

    let report = alice
        .get(app.url(&format!(
            "/api/projects/alice/house/ci/runs/{id}/report.html"
        )))
        .send()
        .await
        .unwrap();
    assert!(report.status().is_success());
    let html = report.text().await.unwrap();
    assert!(html.contains("Schema validation"));
    assert!(html.contains("Passed"));
}

#[tokio::test]
async fn another_user_cannot_read_the_runs() {
    let (app, _alice, head, _at, project) = setup().await;
    let id = insert_check_run(&app, project, &head).await;
    let bob = app.user("bob").await;
    assert_eq!(
        bob.get(app.url("/api/projects/alice/house/ci/runs"))
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    assert_eq!(
        bob.get(app.url(&format!("/api/projects/alice/house/ci/runs/{id}")))
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    assert_eq!(
        bob.get(app.url(&format!(
            "/api/projects/alice/house/ci/runs/{id}/report.html"
        )))
        .send()
        .await
        .unwrap()
        .status(),
        404
    );
}

#[tokio::test]
async fn the_rev_filter_narrows_the_list() {
    let (app, alice, head, _at, project) = setup().await;
    insert_check_run(&app, project, &head).await;
    let list: Value = alice
        .get(app.url(&format!("/api/projects/alice/house/ci/runs?rev={head}")))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(list["runs"].as_array().unwrap().len(), 1);
    let list: Value =
        alice
            .get(app.url(
                "/api/projects/alice/house/ci/runs?rev=0000000000000000000000000000000000000000",
            ))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
    assert_eq!(list["runs"].as_array().unwrap().len(), 0);
}
