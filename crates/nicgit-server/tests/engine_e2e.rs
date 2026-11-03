// SPDX-License-Identifier: AGPL-3.0-or-later
//! End to end with the real KimBim engine worker (NICGIT_TEST_ENGINE=1).
mod common;
use common::engine;
use serde_json::Value;

#[tokio::test]
async fn push_summarises_every_new_head() {
    if !engine::enabled() {
        return eprintln!("skipped: set NICGIT_TEST_ENGINE=1");
    }
    let app = common::TestApp::spawn().await;
    let alice = engine::push_m5(&app).await;
    engine::drain(&app).await;
    let v: Value = alice.get(app.url("/api/projects/alice/m5/versions?option=Scheme%2520B")).send().await.unwrap().json().await.unwrap();
    let head = &v[0];
    assert_eq!(head["summary"]["first"], false, "{head}");
    assert!(head["summary"]["modified"].as_i64().unwrap() >= 1, "{head}");
}
