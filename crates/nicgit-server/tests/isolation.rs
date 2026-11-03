// SPDX-License-Identifier: AGPL-3.0-or-later
//! A private project must not exist for anyone outside it: every route answers 404 (spec section 7).
mod common;

/// Every project-scoped GET route. Add new routes here when they are written.
pub const PROJECT_GET_ROUTES: &[&str] = &[
    "",
    "/members",
    "/overview",
    "/options",
    "/versions",
    "/tags",
    "/files",
    "/activity",
    "/proposals",
    "/issues",
    "/milestones",
    "/rules",
];

#[tokio::test]
async fn foreign_user_gets_404_everywhere() {
    let app = common::TestApp::spawn().await;
    let alice = app.user("alice").await;
    let mallory = app.user("mallory").await;
    app.project(&alice, "alice", "secret", "private").await;
    let anon = reqwest::Client::new();
    for route in PROJECT_GET_ROUTES {
        let url = app.url(&format!("/api/projects/alice/secret{route}"));
        assert_eq!(mallory.get(&url).send().await.unwrap().status(), 404, "mallory GET {route}");
        assert_eq!(anon.get(&url).send().await.unwrap().status(), 404, "anonymous GET {route}");
        // The owner can reach it (or at least not get 404 for an existing project).
        assert_ne!(alice.get(&url).send().await.unwrap().status(), 404, "alice GET {route}");
    }
    let list: serde_json::Value = mallory.get(app.url("/api/projects")).send().await.unwrap().json().await.unwrap();
    assert!(list.as_array().unwrap().is_empty());
    let explore: serde_json::Value = anon.get(app.url("/api/explore")).send().await.unwrap().json().await.unwrap();
    assert!(explore.as_array().unwrap().is_empty());
    // Writes too.
    let r = mallory
        .patch(app.url("/api/projects/alice/secret"))
        .json(&serde_json::json!({"visibility": "public"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 404);
}
