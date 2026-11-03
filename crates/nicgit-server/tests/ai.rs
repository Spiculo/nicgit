// SPDX-License-Identifier: AGPL-3.0-or-later
//! AI through the cloud AI router, budgets and usage.
mod common;
use axum::{routing::post, Json, Router};
use common::git::{git_ok, kimbim_repo, write_record};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

/// A stand-in for the AI router: answers every chat with a fixed text and records the requests.
async fn mock_router() -> (String, Arc<Mutex<Vec<(Option<String>, Value)>>>) {
    let seen: Arc<Mutex<Vec<(Option<String>, Value)>>> = Arc::default();
    let s2 = seen.clone();
    let app = Router::new().route(
        "/v1/chat",
        post(move |headers: axum::http::HeaderMap, Json(body): Json<Value>| {
            let s3 = s2.clone();
            async move {
                let auth = headers.get("authorization").and_then(|v| v.to_str().ok()).map(str::to_string);
                s3.lock().unwrap().push((auth, body));
                Json(json!({"text": "Scheme B adds one wall on Level 1.", "usage": {"units": 42}}))
            }
        }),
    );
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", l.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
    (url, seen)
}

/// alice/house with a proposal scheme-b -> main, and a finished compare for it.
async fn proposal(app: &common::TestApp) -> reqwest::Client {
    let alice = app.user("alice").await;
    let at = app.token(&alice).await;
    let p = app.project(&alice, "alice", "house", "private").await;
    let src = kimbim_repo().await;
    git_ok(src.path(), &["checkout", "-qb", "scheme-b"]).await;
    write_record(src.path(), "objects/cd/1111111111111111111111.json", "{\"a\":{\"Name\":\"W-2\"},\"t\":\"IfcWall\"}");
    git_ok(src.path(), &["add", "-A"]).await;
    git_ok(src.path(), &["commit", "-qm", "Scheme B: second wall"]).await;
    let url = format!("http://git:{at}@{}/alice/house.git", app.base.trim_start_matches("http://"));
    git_ok(src.path(), &["push", &url, "main", "scheme-b"]).await;
    let r: Value = alice
        .post(app.url("/api/projects/alice/house/proposals"))
        .json(&json!({"title": "Scheme B", "source": "scheme-b", "target": "main"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    // The engine's compare, as a worker would have left it.
    let pid = p["id"].as_str().unwrap();
    let key = format!("{pid}:compare:{}:{}", r["target_head"].as_str().unwrap(), r["source_head"].as_str().unwrap());
    sqlx::query("INSERT INTO jobs (id, project_id, kind, args, cache_key, status, result) VALUES ($1, $2, 'compare', '{}', $3, 'done', $4)")
        .bind(uuid::Uuid::new_v4())
        .bind(uuid::Uuid::parse_str(pid).unwrap())
        .bind(key)
        .bind(json!({"details": [{"guid": "1111111111111111111111", "type": "IfcWall", "name": "W-2", "kind": "added", "level": "Level 1", "attributes": [], "properties": []}],
                     "categories": {"IfcWall": {"added": 1, "modified": 0, "removed": 0}}, "tree": {}, "other": 0, "settings": []}))
        .execute(&app.db)
        .await
        .unwrap();
    alice
}

#[tokio::test]
async fn ai_disabled_by_default() {
    let (router, seen) = mock_router().await;
    let app = common::TestApp::spawn_with(move |c| c.ai_router_url = Some(router)).await;
    let alice = proposal(&app).await;
    let r = alice.post(app.url("/api/projects/alice/house/proposals/1/ai-summary")).send().await.unwrap();
    assert_eq!(r.status(), 409);
    assert_eq!(r.json::<Value>().await.unwrap()["error"], "ai_disabled");
    assert!(seen.lock().unwrap().is_empty(), "nothing leaves NicGit while AI is off");
}

#[tokio::test]
async fn ai_summary_calls_router_and_meters() {
    let (router, seen) = mock_router().await;
    let app = common::TestApp::spawn_with(move |c| {
        c.ai_router_url = Some(router);
        c.ai_router_key = Some("router-secret".into());
    })
    .await;
    let alice = proposal(&app).await;
    let r = alice.put(app.url("/api/accounts/alice/ai")).json(&json!({"enabled": true})).send().await.unwrap();
    assert_eq!(r.status(), 200);
    let r = alice.post(app.url("/api/projects/alice/house/proposals/1/ai-summary")).send().await.unwrap();
    assert_eq!(r.status(), 200, "{}", r.text().await.unwrap());
    let v: Value = r.json().await.unwrap();
    assert_eq!(v["text"], "Scheme B adds one wall on Level 1.");
    {
        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].0.as_deref(), Some("Bearer router-secret"));
        let body = seen[0].1.to_string();
        assert!(body.contains("W-2"), "the structured diff is sent: {body}");
        assert_eq!(seen[0].1["metadata"]["purpose"], "proposal_summary");
    }
    // Cached on the proposal; the router is not called again.
    let d: Value = alice.get(app.url("/api/projects/alice/house/proposals/1")).send().await.unwrap().json().await.unwrap();
    assert_eq!(d["ai_summary"]["text"], "Scheme B adds one wall on Level 1.");
    let usage: Value = alice.get(app.url("/api/accounts/alice/usage")).send().await.unwrap().json().await.unwrap();
    let ai = usage["usage"].as_array().unwrap().iter().find(|u| u["kind"] == "ai_units").unwrap();
    assert_eq!(ai["quantity"], 42);
}

#[tokio::test]
async fn budget_blocks_ai_when_hard_limit() {
    let (router, seen) = mock_router().await;
    let app = common::TestApp::spawn_with(move |c| c.ai_router_url = Some(router)).await;
    let alice = proposal(&app).await;
    alice.put(app.url("/api/accounts/alice/ai")).json(&json!({"enabled": true})).send().await.unwrap();
    let r = alice.put(app.url("/api/accounts/alice/budgets/ai_units")).json(&json!({"soft_limit": 5, "hard_limit": 10})).send().await.unwrap();
    assert_eq!(r.status(), 200);
    sqlx::query("INSERT INTO usage_events (account_id, kind, quantity) SELECT id, 'ai_units', 10 FROM accounts WHERE slug = 'alice'")
        .execute(&app.db)
        .await
        .unwrap();
    let r = alice.post(app.url("/api/projects/alice/house/proposals/1/ai-summary")).send().await.unwrap();
    assert_eq!(r.status(), 402);
    assert!(seen.lock().unwrap().is_empty());
    // Reading and pushing are never blocked by a budget.
    assert_eq!(alice.get(app.url("/api/projects/alice/house/overview")).send().await.unwrap().status(), 200);
}

#[tokio::test]
async fn ai_unavailable_without_router() {
    let app = common::TestApp::spawn().await;
    let alice = proposal(&app).await;
    alice.put(app.url("/api/accounts/alice/ai")).json(&json!({"enabled": true})).send().await.unwrap();
    let r = alice.post(app.url("/api/projects/alice/house/proposals/1/ai-summary")).send().await.unwrap();
    assert_eq!(r.status(), 409);
    assert_eq!(r.json::<Value>().await.unwrap()["error"], "ai_unavailable");
}

#[tokio::test]
async fn usage_reports_storage_and_is_private() {
    let app = common::TestApp::spawn().await;
    let alice = proposal(&app).await;
    let mut storage = 0;
    for _ in 0..50 {
        let usage: Value = alice.get(app.url("/api/accounts/alice/usage")).send().await.unwrap().json().await.unwrap();
        storage = usage["usage"].as_array().unwrap().iter().find(|u| u["kind"] == "storage_bytes").map(|u| u["quantity"].as_i64().unwrap()).unwrap_or(0);
        if storage > 0 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert!(storage > 0);
    let mallory = app.user("mallory").await;
    assert_eq!(mallory.get(app.url("/api/accounts/alice/usage")).send().await.unwrap().status(), 404);
    assert_eq!(mallory.put(app.url("/api/accounts/alice/ai")).json(&json!({"enabled": true})).send().await.unwrap().status(), 404);
}
