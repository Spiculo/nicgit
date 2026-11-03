// SPDX-License-Identifier: AGPL-3.0-or-later
mod common;

#[tokio::test]
async fn health_is_ok() {
    let app = common::TestApp::spawn().await;
    let v: serde_json::Value = app.client.get(app.url("/api/health")).send().await.unwrap().json().await.unwrap();
    assert_eq!(v["ok"], true);
}
