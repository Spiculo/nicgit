// SPDX-License-Identifier: AGPL-3.0-or-later
//! End to end with the real NicBim engine worker (NICGIT_TEST_ENGINE=1).
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

#[tokio::test]
async fn kim_import_becomes_a_browsable_project() {
    if !engine::enabled() {
        return eprintln!("skipped: set NICGIT_TEST_ENGINE=1");
    }
    let app = common::TestApp::spawn().await;
    let alice = app.user("alice").await;
    // A .nic of the M5 project, packed by the engine itself.
    let tmp = tempfile::tempdir().unwrap();
    let kim = tmp.path().join("m5.nic");
    let script = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scripts/engine-python.sh");
    let fx = engine::m5_fixture();
    let code = format!(
        "import subprocess,pathlib,sys\nfrom kimbim import kimfile\nf=pathlib.Path(sys.argv[2])/'p'\nsubprocess.run(['git','clone','--bare','-q',sys.argv[1],str(f/'.git')],check=True)\nkimfile.pack(f, sys.argv[3])"
    );
    let out = tokio::process::Command::new("sh")
        .arg(&script)
        .args(["-c", &code, &fx.to_string_lossy(), &tmp.path().to_string_lossy(), &kim.to_string_lossy()])
        .output()
        .await
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let part = reqwest::multipart::Part::bytes(std::fs::read(&kim).unwrap()).file_name("m5.nic");
    let form = reqwest::multipart::Form::new().text("owner", "alice").text("slug", "m5").part("file", part);
    let r = alice.post(app.url("/api/projects/import")).multipart(form).send().await.unwrap();
    assert_eq!(r.status(), 200, "{}", r.text().await.unwrap());
    engine::drain(&app).await; // import
    engine::drain(&app).await; // summaries queued by the import
    let overview: Value = alice.get(app.url("/api/projects/alice/m5/overview")).send().await.unwrap().json().await.unwrap();
    assert_eq!(overview["empty"], false, "{overview}");
    assert_eq!(overview["project"]["default_option"], "main");
    assert_eq!(overview["options"].as_array().unwrap().len(), 2);
    assert!(overview["latest"]["summary"].is_object(), "{overview}");
    let feed: Value = alice.get(app.url("/api/projects/alice/m5/activity")).send().await.unwrap().json().await.unwrap();
    assert!(feed.as_array().unwrap().iter().any(|a| a["kind"] == "project.imported"), "{feed}");
}
