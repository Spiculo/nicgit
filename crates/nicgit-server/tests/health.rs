// SPDX-License-Identifier: AGPL-3.0-or-later
mod common;

#[tokio::test]
async fn health_is_ok() {
    let app = common::TestApp::spawn().await;
    let v: serde_json::Value = app.client.get(app.url("/api/health")).send().await.unwrap().json().await.unwrap();
    assert_eq!(v["ok"], true);
}

#[tokio::test]
async fn instance_says_what_this_server_is() {
    let app = common::TestApp::spawn_with(|c| {
        c.deployment = nicgit_server::config::Deployment::SelfHosted;
        c.signup.open = false;
    })
    .await;
    let v: serde_json::Value = app.client.get(app.url("/api/instance")).send().await.unwrap().json().await.unwrap();
    assert_eq!(v["deployment"], "self-hosted");
    assert_eq!(v["signup"], "closed");
    assert_eq!(v["version"], env!("CARGO_PKG_VERSION"));
    assert!(v["engine"].is_string());
}

#[tokio::test]
async fn serves_the_web_app_with_history_fallback() {
    let web = tempfile::tempdir().unwrap();
    std::fs::write(web.path().join("index.html"), "<!doctype html><title>NicGit</title>").unwrap();
    std::fs::create_dir_all(web.path().join("wasm")).unwrap();
    std::fs::write(web.path().join("wasm/web-ifc.wasm"), b"\0asm").unwrap();
    let dir = web.path().to_path_buf();
    let app = common::TestApp::spawn_with(move |c| c.web_dir = Some(dir)).await;
    for path in ["/", "/alice/house/versions", "/login"] {
        let r = app.client.get(app.url(path)).send().await.unwrap();
        assert_eq!(r.status(), 200, "{path}");
        assert!(r.text().await.unwrap().contains("<title>NicGit</title>"), "{path}");
    }
    let r = app.client.get(app.url("/wasm/web-ifc.wasm")).send().await.unwrap();
    assert_eq!(r.headers()["content-type"], "application/wasm");
    // Unknown API routes stay JSON 404s, never the app.
    assert_eq!(app.client.get(app.url("/api/nope")).send().await.unwrap().status(), 404);
}
