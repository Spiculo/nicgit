// SPDX-License-Identifier: AGPL-3.0-or-later
mod common;
use nicgit_server::jobs;
use serde_json::{json, Value};
use uuid::Uuid;

async fn project_id(app: &common::TestApp, c: &reqwest::Client) -> Uuid {
    let p = app.project(c, "alice", "house", "private").await;
    Uuid::parse_str(p["id"].as_str().unwrap()).unwrap()
}

#[tokio::test]
async fn enqueue_dedupes_by_cache_key() {
    let app = common::TestApp::spawn().await;
    let alice = app.user("alice").await;
    let pid = project_id(&app, &alice).await;
    let a = jobs::enqueue(&app.db, pid, "summary", json!({"rev": "x"}), Some("k1")).await.unwrap();
    let b = jobs::enqueue(&app.db, pid, "summary", json!({"rev": "x"}), Some("k1")).await.unwrap();
    assert_eq!(a.id, b.id);
    let c = jobs::enqueue(&app.db, pid, "summary", json!({"rev": "y"}), None).await.unwrap();
    assert_ne!(a.id, c.id);
}

#[tokio::test]
async fn internal_requires_token() {
    let app = common::TestApp::spawn().await;
    let c = reqwest::Client::new();
    let r = c.post(app.url("/internal/jobs/claim")).json(&json!({"kinds": ["summary"]})).send().await.unwrap();
    assert_eq!(r.status(), 401);
    let r = c
        .post(app.url("/internal/jobs/claim"))
        .header("x-worker-token", "wrong")
        .json(&json!({"kinds": ["summary"]}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 401);
}

#[tokio::test]
async fn claim_finish_and_read() {
    let app = common::TestApp::spawn().await;
    let alice = app.user("alice").await;
    let pid = project_id(&app, &alice).await;
    let job = jobs::enqueue(&app.db, pid, "summary", json!({"rev": "abc"}), None).await.unwrap();
    let w = reqwest::Client::new();
    let claim = |kinds: Value| {
        w.post(app.url("/internal/jobs/claim")).header("x-worker-token", common::WORKER_TOKEN).json(&json!({"kinds": kinds})).send()
    };
    // A worker that does not handle the kind gets nothing.
    assert_eq!(claim(json!(["compare"])).await.unwrap().status(), 204);
    let got: Value = claim(json!(["summary"])).await.unwrap().json().await.unwrap();
    assert_eq!(got["id"], job.id.to_string());
    assert_eq!(got["args"]["rev"], "abc");
    assert!(got["repo"].as_str().unwrap().ends_with(&pid.to_string()));
    assert!(got["cache_dir"].as_str().unwrap().contains(&pid.to_string()));
    // Claimed jobs are not handed out twice.
    assert_eq!(claim(json!(["summary"])).await.unwrap().status(), 204);
    let r = w
        .post(app.url(&format!("/internal/jobs/{}/finish", job.id)))
        .header("x-worker-token", common::WORKER_TOKEN)
        .json(&json!({"result": {"added": 3}}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let seen: Value = alice.get(app.url(&format!("/api/jobs/{}", job.id))).send().await.unwrap().json().await.unwrap();
    assert_eq!(seen["status"], "done");
    assert_eq!(seen["result"]["added"], 3);
    let mallory = app.user("mallory").await;
    assert_eq!(mallory.get(app.url(&format!("/api/jobs/{}", job.id))).send().await.unwrap().status(), 404);
}

#[tokio::test]
async fn failed_job_is_requeued_on_enqueue() {
    let app = common::TestApp::spawn().await;
    let alice = app.user("alice").await;
    let pid = project_id(&app, &alice).await;
    let job = jobs::enqueue(&app.db, pid, "summary", json!({}), Some("k2")).await.unwrap();
    let w = reqwest::Client::new();
    w.post(app.url("/internal/jobs/claim")).header("x-worker-token", common::WORKER_TOKEN).json(&json!({"kinds": ["summary"]})).send().await.unwrap();
    w.post(app.url(&format!("/internal/jobs/{}/finish", job.id)))
        .header("x-worker-token", common::WORKER_TOKEN)
        .json(&json!({"error": "boom"}))
        .send()
        .await
        .unwrap();
    let again = jobs::enqueue(&app.db, pid, "summary", json!({}), Some("k2")).await.unwrap();
    assert_eq!(again.id, job.id);
    assert_eq!(again.status, "queued");
}

#[tokio::test]
async fn long_job_with_heartbeat_is_not_handed_out_again() {
    let app = common::TestApp::spawn().await;
    let alice = app.user("alice").await;
    let pid = project_id(&app, &alice).await;
    let job = jobs::enqueue(&app.db, pid, "import_ifc", json!({}), None).await.unwrap();
    let w = reqwest::Client::new();
    let claim = || w.post(app.url("/internal/jobs/claim")).header("x-worker-token", common::WORKER_TOKEN).json(&json!({"kinds": ["import_ifc"]})).send();
    assert_eq!(claim().await.unwrap().status(), 200);
    // Started an hour ago, but its worker reported progress a moment ago: still alive.
    sqlx::query("UPDATE jobs SET started_at = now() - interval '1 hour' WHERE id = $1").bind(job.id).execute(&app.db).await.unwrap();
    w.post(app.url(&format!("/internal/jobs/{}/progress", job.id)))
        .header("x-worker-token", common::WORKER_TOKEN)
        .json(&json!({"progress": 0.5}))
        .send()
        .await
        .unwrap();
    assert_eq!(claim().await.unwrap().status(), 204);
    // Silent for longer than the heartbeat window: its worker is gone, hand it out again.
    sqlx::query("UPDATE jobs SET heartbeat_at = now() - interval '20 minutes' WHERE id = $1").bind(job.id).execute(&app.db).await.unwrap();
    assert_eq!(claim().await.unwrap().status(), 200);
}
