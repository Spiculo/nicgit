// SPDX-License-Identifier: AGPL-3.0-or-later
//! Actions: the checks a project asks for, run by engine workers, metered and budgeted.
mod common;
use common::git::{git_ok, kimbim_repo, write_record};
use serde_json::{json, Value};

async fn setup() -> (common::TestApp, reqwest::Client, tempfile::TempDir, String) {
    let app = common::TestApp::spawn().await;
    let alice = app.user("alice").await;
    let at = app.token(&alice).await;
    app.project(&alice, "alice", "house", "private").await;
    let r = alice
        .put(app.url("/api/projects/alice/house/rules/main"))
        .json(&json!({"required_approvals": 0, "required_checks": ["schema"]}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let src = kimbim_repo().await;
    let url = format!("http://git:{at}@{}/alice/house.git", app.base.trim_start_matches("http://"));
    git_ok(src.path(), &["push", &url, "main"]).await;
    (app, alice, src, url)
}

async fn jobs_of(app: &common::TestApp, kind: &str) -> Vec<(uuid::Uuid, Value)> {
    sqlx::query_as("SELECT id, args FROM jobs WHERE kind = $1").bind(kind).fetch_all(&app.db).await.unwrap()
}

async fn wait_jobs(app: &common::TestApp, kind: &str, n: usize) -> Vec<(uuid::Uuid, Value)> {
    for _ in 0..50 {
        let j = jobs_of(app, kind).await;
        if j.len() >= n {
            return j;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    jobs_of(app, kind).await
}

#[tokio::test]
async fn push_to_option_with_required_checks_queues_action() {
    let (app, _alice, _src, _url) = setup().await;
    let jobs = wait_jobs(&app, "checks", 1).await;
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].1["checks"], json!(["schema"]));
}

#[tokio::test]
async fn required_checks_block_combine() {
    let (app, alice, src, url) = setup().await;
    git_ok(src.path(), &["checkout", "-qb", "scheme-b"]).await;
    write_record(src.path(), "objects/cd/1111111111111111111111.json", "{\"a\":{\"Name\":\"W-2\"},\"t\":\"IfcWall\"}");
    git_ok(src.path(), &["add", "-A"]).await;
    git_ok(src.path(), &["commit", "-qm", "two"]).await;
    git_ok(src.path(), &["push", &url, "scheme-b"]).await;
    alice
        .post(app.url("/api/projects/alice/house/proposals"))
        .json(&json!({"title": "B", "source": "scheme-b", "target": "main"}))
        .send()
        .await
        .unwrap();
    let d: Value = alice.get(app.url("/api/projects/alice/house/proposals/1")).send().await.unwrap().json().await.unwrap();
    assert_eq!(d["checks"]["required"], json!(["schema"]));
    assert_eq!(d["checks"]["state"], "pending");
    let r = alice.post(app.url("/api/projects/alice/house/proposals/1/combine")).json(&json!({"mode": "auto"})).send().await.unwrap();
    assert_eq!(r.status(), 409);
    assert_eq!(r.json::<Value>().await.unwrap()["error"], "checks_pending");
}

#[tokio::test]
async fn finished_checks_meter_action_seconds() {
    let (app, alice, _src, _url) = setup().await;
    wait_jobs(&app, "checks", 1).await;
    let job = app.claim(&["checks"]).await;
    assert_eq!(app.finish(&job, json!({"result": {"checks": [], "passed": true, "seconds": 3.2}})).await, 200);
    let usage: Value = alice.get(app.url("/api/accounts/alice/usage")).send().await.unwrap().json().await.unwrap();
    let a = usage["usage"].as_array().unwrap().iter().find(|u| u["kind"] == "action_seconds").unwrap();
    assert_eq!(a["quantity"], 4);
    let v: Value = alice.get(app.url("/api/projects/alice/house/versions/main/checks?names=schema")).send().await.unwrap().json().await.unwrap();
    assert_eq!(v["passed"], true);
}

#[tokio::test]
async fn action_budget_blocks_new_runs() {
    let (app, alice, _src, _url) = setup().await;
    alice.put(app.url("/api/accounts/alice/budgets/action_seconds")).json(&json!({"hard_limit": 5})).send().await.unwrap();
    sqlx::query("INSERT INTO usage_events (account_id, kind, quantity) SELECT id, 'action_seconds', 5 FROM accounts WHERE slug = 'alice'")
        .execute(&app.db)
        .await
        .unwrap();
    let r = alice.get(app.url("/api/projects/alice/house/versions/main/checks?names=model")).send().await.unwrap();
    assert_eq!(r.status(), 402);
    let r = alice.get(app.url("/api/projects/alice/house/versions/main/checks?names=bogus")).send().await.unwrap();
    assert_eq!(r.status(), 400);
}

#[tokio::test]
async fn viewing_a_proposal_never_spends_actions() {
    let (app, alice, src, url) = setup().await;
    alice.patch(app.url("/api/projects/alice/house")).json(&json!({"visibility": "public"})).send().await.unwrap();
    git_ok(src.path(), &["checkout", "-qb", "scheme-b"]).await;
    write_record(src.path(), "objects/cd/1111111111111111111111.json", "{\"a\":{\"Name\":\"W-2\"},\"t\":\"IfcWall\"}");
    git_ok(src.path(), &["add", "-A"]).await;
    git_ok(src.path(), &["commit", "-qm", "two"]).await;
    git_ok(src.path(), &["push", &url, "scheme-b"]).await;
    alice.post(app.url("/api/projects/alice/house/proposals")).json(&json!({"title": "B", "source": "scheme-b", "target": "main"})).send().await.unwrap();
    let before = wait_jobs(&app, "checks", 2).await.len();
    sqlx::query("DELETE FROM jobs WHERE kind = 'checks'").execute(&app.db).await.unwrap();
    let anon = reqwest::Client::new();
    for _ in 0..3 {
        assert_eq!(anon.get(app.url("/api/projects/alice/house/proposals/1")).send().await.unwrap().status(), 200);
    }
    assert_eq!(before, 2, "push to main and the proposal's source head each queue checks once");
    assert!(jobs_of(&app, "checks").await.is_empty(), "a page view queued an Action");
}
