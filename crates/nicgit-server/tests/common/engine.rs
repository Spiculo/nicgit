// SPDX-License-Identifier: AGPL-3.0-or-later
//! Running the real Python engine worker in tests. These tests are skipped unless NICGIT_TEST_ENGINE=1
//! (they need nicbim's pixi environment and its M5 fixture).
use super::TestApp;
use std::path::PathBuf;

pub fn enabled() -> bool {
    std::env::var("NICGIT_TEST_ENGINE").is_ok_and(|v| v == "1")
}

pub fn m5_fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../nicbim/build/m5/project/.git")
}

/// Run one worker until the queue is empty.
pub async fn drain(app: &TestApp) {
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scripts/engine-python.sh");
    let out = tokio::process::Command::new("sh")
        .arg(script)
        .args(["-m", "nicgit_worker", "--once", "--url", &app.base])
        .env("NICGIT_WORKER_TOKEN", super::WORKER_TOKEN)
        .output()
        .await
        .expect("worker runs");
    assert!(
        out.status.success(),
        "worker failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Push the M5 project (all options) to alice/m5; returns alice's client.
pub async fn push_m5(app: &TestApp) -> reqwest::Client {
    let alice = app.user("alice").await;
    let at = app.token(&alice).await;
    app.project(&alice, "alice", "m5", "private").await;
    let url = format!(
        "http://git:{at}@{}/alice/m5.git",
        app.base.trim_start_matches("http://")
    );
    let fx = m5_fixture().to_string_lossy().to_string();
    super::git::git_ok(
        std::path::Path::new("."),
        &["--git-dir", &fx, "push", &url, "refs/heads/*:refs/heads/*"],
    )
    .await;
    wait_for_push(app).await;
    alice
}

/// Push handling runs after git finishes; wait until it has recorded the push.
pub async fn wait_for_push(app: &TestApp) {
    for _ in 0..100 {
        let n: i64 = sqlx::query_scalar("SELECT count(*) FROM activity WHERE kind = 'push'")
            .fetch_one(&app.db)
            .await
            .unwrap();
        if n > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    panic!("push was never processed");
}
