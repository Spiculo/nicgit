// SPDX-License-Identifier: AGPL-3.0-or-later
//! What keeps open signup from hurting everyone else (decisions N33-N36): storage limits, fair use of workers,
//! user names and addresses that stay private, and the workers' API off the public address.
mod common;
use common::git::{git, git_ok, kimbim_repo, write_record};
use nicgit_server::{app, config::Config, jobs};
use serde_json::{json, Value};
use std::path::Path;
use std::sync::Arc;
use uuid::Uuid;

fn push_url(app: &common::TestApp, user: &str, token: &str, owner: &str, repo: &str) -> String {
    format!("http://{user}:{token}@{}/{owner}/{repo}.git", app.base.trim_start_matches("http://"))
}

/// Bytes git cannot compress, so what lands on disk is about this size.
fn noise(n: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(n + 32);
    while out.len() < n {
        out.extend_from_slice(Uuid::new_v4().as_bytes());
    }
    out.truncate(n);
    out
}

async fn commit_file(repo: &Path, name: &str, n: usize) {
    std::fs::write(repo.join(name), noise(n)).unwrap();
    git_ok(repo, &["add", "-A"]).await;
    git_ok(repo, &["commit", "-qm", name]).await;
}

async fn setup(tweak: impl FnOnce(&mut Config)) -> (common::TestApp, reqwest::Client, String, String, tempfile::TempDir) {
    let app = common::TestApp::spawn_with(tweak).await;
    let alice = app.user("alice").await;
    app.project(&alice, "alice", "house", "private").await;
    let token = app.token(&alice).await;
    let url = push_url(&app, "alice", &token, "alice", "house");
    let repo = kimbim_repo().await;
    (app, alice, token, url, repo)
}

fn stderr(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn import_form(owner: &str, slug: &str, file: Vec<u8>) -> reqwest::multipart::Form {
    reqwest::multipart::Form::new()
        .text("owner", owner.to_string())
        .text("slug", slug.to_string())
        .part("file", reqwest::multipart::Part::bytes(file).file_name("model.ifc"))
}

// ---- one file ----

#[tokio::test]
async fn a_push_with_a_file_over_the_limit_is_refused_and_says_which() {
    let (_app, _alice, _t, url, repo) = setup(|c| c.max_file_mb = 1).await;
    commit_file(repo.path(), "big.bin", (1 << 20) + 10).await;
    let out = git(repo.path(), &["push", "-q", &url, "main"]).await;
    assert!(!out.status.success());
    let msg = stderr(&out);
    assert!(msg.contains("big.bin") && msg.contains("1 MB"), "{msg}");
    assert_eq!(git_ok(repo.path(), &["ls-remote", &url]).await, "", "no ref moved");
}

#[tokio::test]
async fn a_huge_file_that_compresses_to_nothing_is_still_refused() {
    // Small on the wire, enormous once read: this must not reach the check that reads records into memory.
    let (_app, _alice, _t, url, repo) = setup(|c| c.max_file_mb = 1).await;
    write_record(repo.path(), "objects/ab/huge.json", &" ".repeat(8 << 20));
    git_ok(repo.path(), &["add", "-A"]).await;
    git_ok(repo.path(), &["commit", "-qm", "bomb"]).await;
    let out = git(repo.path(), &["push", "-q", &url, "main"]).await;
    assert!(!out.status.success());
    let msg = stderr(&out);
    assert!(msg.contains("objects/ab/huge.json") && msg.contains("larger than"), "{msg}");
}

#[tokio::test]
async fn a_file_within_the_limit_is_pushed() {
    let (_app, _alice, _t, url, repo) = setup(|c| c.max_file_mb = 1).await;
    commit_file(repo.path(), "ok.bin", (1 << 20) - 10).await;
    git_ok(repo.path(), &["push", "-q", &url, "main"]).await;
}

#[tokio::test]
async fn an_upload_over_the_file_limit_is_refused_and_nothing_is_kept() {
    let (app, alice, _t, _u, _r) = setup(|c| c.max_file_mb = 1).await;
    let r = alice.post(app.url("/api/projects/import")).multipart(import_form("alice", "big", noise((1 << 20) + 10))).send().await.unwrap();
    assert_eq!(r.status(), 413);
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM projects WHERE slug = 'big'").fetch_one(&app.db).await.unwrap();
    assert_eq!(n, 0);
    let left = std::fs::read_dir(app.data.path().join("uploads").join("incoming")).unwrap().count();
    assert_eq!(left, 0);
}

// ---- one project ----

#[tokio::test]
async fn a_pack_larger_than_the_room_left_in_the_project_is_refused() {
    let (_app, _alice, _t, url, repo) = setup(|c| c.max_repo_mb = 1).await;
    commit_file(repo.path(), "big.bin", 2 << 20).await;
    let out = git(repo.path(), &["push", "-q", &url, "main"]).await;
    assert!(!out.status.success(), "the push went through");
    assert!(stderr(&out).contains("maximum allowed size"), "{}", stderr(&out));
    assert_eq!(git_ok(repo.path(), &["ls-remote", &url]).await, "", "no ref moved");
}

#[tokio::test]
async fn a_full_project_refuses_pushes_with_the_reason_but_can_still_be_read() {
    let (app, alice, _t, url, repo) = setup(|c| {
        c.max_repo_mb = 1;
        c.max_file_mb = 10;
    })
    .await;
    git_ok(repo.path(), &["push", "-q", &url, "main"]).await;
    // Fill it as past pushes would have.
    let pid: Uuid = sqlx::query_scalar("SELECT id FROM projects WHERE slug = 'house'").fetch_one(&app.db).await.unwrap();
    std::fs::write(app.data.path().join("repos").join(pid.to_string()).join(".git").join("filler"), noise(2 << 20)).unwrap();
    write_record(repo.path(), "settings.json", "{\"units\":\"m\"}");
    git_ok(repo.path(), &["add", "-A"]).await;
    git_ok(repo.path(), &["commit", "-qm", "more"]).await;
    let out = git(repo.path(), &["push", "-q", &url, "main"]).await;
    assert!(!out.status.success());
    let msg = stderr(&out);
    assert!(msg.contains("this project has reached its 1 MB limit"), "{msg}");
    let elsewhere = tempfile::tempdir().unwrap();
    git_ok(elsewhere.path(), &["clone", "-q", &url, "copy"]).await;
    assert_eq!(alice.get(app.url("/api/projects/alice/house")).send().await.unwrap().status(), 200);
}

// ---- one account ----

#[tokio::test]
async fn projects_add_up_to_the_account_limit() {
    let (app, alice, token, url, repo) = setup(|c| {
        c.max_repo_mb = 3;
        c.max_file_mb = 10;
        c.max_account_mb = 3;
    })
    .await;
    commit_file(repo.path(), "a.bin", 2 << 20).await;
    git_ok(repo.path(), &["push", "-q", &url, "main"]).await;
    let usage: Value = alice.get(app.url("/api/me/storage")).send().await.unwrap().json().await.unwrap();
    assert!(usage["used"].as_u64().unwrap() >= 2 << 20, "{usage}");
    assert_eq!(usage["limit"], 3u64 << 20);
    // A second project has plenty of its own room, but the account does not.
    app.project(&alice, "alice", "shed", "private").await;
    let other = kimbim_repo().await;
    commit_file(other.path(), "b.bin", 2 << 20).await;
    let out = git(other.path(), &["push", "-q", &push_url(&app, "alice", &token, "alice", "shed"), "main"]).await;
    assert!(!out.status.success(), "the push went through");
    assert!(stderr(&out).contains("maximum allowed size"), "{}", stderr(&out));
}

#[tokio::test]
async fn an_upload_that_does_not_fit_the_account_is_refused_before_a_project_exists() {
    let (app, alice, _t, url, repo) = setup(|c| {
        c.max_repo_mb = 3;
        c.max_file_mb = 2;
        c.max_account_mb = 3;
    })
    .await;
    commit_file(repo.path(), "a.bin", (2 << 20) - 10).await;
    git_ok(repo.path(), &["push", "-q", &url, "main"]).await;
    let r = alice.post(app.url("/api/projects/import")).multipart(import_form("alice", "more", noise((2 << 20) - 10))).send().await.unwrap();
    assert_eq!(r.status(), 413);
    let v: Value = r.json().await.unwrap();
    assert_eq!(v["error"], "storage_full");
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM projects WHERE slug = 'more'").fetch_one(&app.db).await.unwrap();
    assert_eq!(n, 0);
    assert_eq!(std::fs::read_dir(app.data.path().join("uploads").join("incoming")).unwrap().count(), 0);
}

#[tokio::test]
async fn a_stranger_cannot_learn_whether_an_account_is_full() {
    let (app, _alice, _t, url, repo) = setup(|c| {
        c.max_repo_mb = 3;
        c.max_file_mb = 2;
        c.max_account_mb = 3;
    })
    .await;
    commit_file(repo.path(), "a.bin", (2 << 20) - 10).await;
    git_ok(repo.path(), &["push", "-q", &url, "main"]).await;
    let bob = app.user("bob").await;
    let r = bob.post(app.url("/api/projects/import")).multipart(import_form("alice", "x", noise((2 << 20) - 10))).send().await.unwrap();
    assert_eq!(r.status(), 403);
}

#[tokio::test]
async fn the_number_of_projects_and_organisations_is_limited() {
    let app = common::TestApp::spawn_with(|c| {
        c.max_projects = 2;
        c.max_orgs = 1;
    })
    .await;
    let alice = app.user("alice").await;
    app.project(&alice, "alice", "one", "private").await;
    app.project(&alice, "alice", "two", "private").await;
    let r = alice.post(app.url("/api/projects")).json(&json!({"owner": "alice", "slug": "three"})).send().await.unwrap();
    assert_eq!(r.status(), 403);
    assert_eq!(r.json::<Value>().await.unwrap()["error"], "limit_reached");
    assert_eq!(alice.post(app.url("/api/orgs")).json(&json!({"slug": "first"})).send().await.unwrap().status(), 200);
    assert_eq!(alice.post(app.url("/api/orgs")).json(&json!({"slug": "second"})).send().await.unwrap().status(), 403);
}

// ---- what jobs store ----

async fn put_artifact(app: &common::TestApp, job: &Value, name: &str, n: usize) -> reqwest::StatusCode {
    reqwest::Client::new()
        .put(app.url(&format!("/internal/jobs/{}/artifacts/ifc/{name}", job["id"].as_str().unwrap())))
        .bearer_auth(job["token"].as_str().unwrap())
        .body(vec![0u8; n])
        .send()
        .await
        .unwrap()
        .status()
}

#[tokio::test]
async fn old_cached_exports_make_way_for_new_ones() {
    let (app, _alice, _t, url, repo) = setup(|c| c.max_repo_mb = 2).await;
    git_ok(repo.path(), &["push", "-q", &url, "main"]).await;
    let pid: Uuid = sqlx::query_scalar("SELECT id FROM projects WHERE slug = 'house'").fetch_one(&app.db).await.unwrap();
    sqlx::query("UPDATE jobs SET status = 'done' WHERE status = 'queued'").execute(&app.db).await.unwrap();
    let dir = app.data.path().join("cache").join(pid.to_string()).join("ifc");
    for (i, name) in ["a.ifc", "b.ifc", "c.ifc"].iter().enumerate() {
        jobs::enqueue(&app.db, pid, "export_ifc", json!({"n": i}), None).await.unwrap();
        let job = app.claim(&["export_ifc"]).await;
        assert_eq!(put_artifact(&app, &job, name, 900 * 1024).await, 200);
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;
    }
    assert!(!dir.join("a.ifc").exists(), "the oldest goes");
    assert!(dir.join("b.ifc").exists() && dir.join("c.ifc").exists());
}

#[tokio::test]
async fn an_import_that_does_not_fit_fails_with_the_reason() {
    let (app, _alice, _t, url, repo) = setup(|c| {
        c.max_repo_mb = 1;
        c.max_file_mb = 10;
    })
    .await;
    git_ok(repo.path(), &["push", "-q", &url, "main"]).await;
    let pid: Uuid = sqlx::query_scalar("SELECT id FROM projects WHERE slug = 'house'").fetch_one(&app.db).await.unwrap();
    std::fs::write(app.data.path().join("repos").join(pid.to_string()).join(".git").join("filler"), noise(2 << 20)).unwrap();
    sqlx::query("UPDATE jobs SET status = 'done' WHERE status = 'queued'").execute(&app.db).await.unwrap();
    jobs::enqueue(&app.db, pid, "import_kim", json!({"upload": "x.kim"}), None).await.unwrap();
    let job = app.claim(&["import_kim"]).await;
    let h = format!("http.extraHeader=Authorization: Bearer {}", job["token"].as_str().unwrap());
    let git_url = app.url(&format!("/internal/jobs/{}/git", job["id"].as_str().unwrap()));
    git_ok(repo.path(), &["checkout", "-q", "-b", "imported"]).await;
    let out = git(repo.path(), &["-c", &h, "push", "-q", &git_url, "imported"]).await;
    assert!(!out.status.success());
    assert!(stderr(&out).contains("this project has reached its 1 MB limit"), "{}", stderr(&out));
}

#[tokio::test]
async fn a_full_project_can_still_finish_a_combine() {
    let (app, _alice, _t, url, repo) = setup(|c| {
        c.max_repo_mb = 1;
        c.max_file_mb = 10;
    })
    .await;
    git_ok(repo.path(), &["push", "-q", &url, "main"]).await;
    let pid: Uuid = sqlx::query_scalar("SELECT id FROM projects WHERE slug = 'house'").fetch_one(&app.db).await.unwrap();
    std::fs::write(app.data.path().join("repos").join(pid.to_string()).join(".git").join("filler"), noise(2 << 20)).unwrap();
    sqlx::query("UPDATE jobs SET status = 'done' WHERE status = 'queued'").execute(&app.db).await.unwrap();
    jobs::enqueue(&app.db, pid, "combine", json!({"target": "main"}), None).await.unwrap();
    let job = app.claim(&["combine"]).await;
    let h = format!("http.extraHeader=Authorization: Bearer {}", job["token"].as_str().unwrap());
    let git_url = app.url(&format!("/internal/jobs/{}/git", job["id"].as_str().unwrap()));
    write_record(repo.path(), "settings.json", "{\"units\":\"m\"}");
    git_ok(repo.path(), &["add", "-A"]).await;
    git_ok(repo.path(), &["commit", "-qm", "combined"]).await;
    git_ok(repo.path(), &["-c", &h, "push", "-q", &git_url, "main"]).await;
}

// ---- fair use of workers ----

#[tokio::test]
async fn an_account_filling_the_queue_does_not_keep_others_waiting() {
    let app = common::TestApp::spawn().await;
    let alice = app.user("alice").await;
    let bob = app.user("bob").await;
    let a = app.project(&alice, "alice", "a", "private").await;
    let b = app.project(&bob, "bob", "b", "private").await;
    let id = |p: &Value| Uuid::parse_str(p["id"].as_str().unwrap()).unwrap();
    for n in 0..5 {
        jobs::enqueue(&app.db, id(&a), "summary", json!({"n": n}), None).await.unwrap();
    }
    jobs::enqueue(&app.db, id(&b), "summary", json!({}), None).await.unwrap();
    let first = app.claim(&["summary"]).await;
    assert_eq!(first["project"], a["id"], "alice was first in the queue");
    let second = app.claim(&["summary"]).await;
    assert_eq!(second["project"], b["id"], "bob goes next, ahead of alice's other four");
}

#[tokio::test]
async fn a_job_that_keeps_losing_its_worker_gives_up() {
    let app = common::TestApp::spawn().await;
    let alice = app.user("alice").await;
    let p = app.project(&alice, "alice", "a", "private").await;
    let pid = Uuid::parse_str(p["id"].as_str().unwrap()).unwrap();
    let job = jobs::enqueue(&app.db, pid, "summary", json!({}), None).await.unwrap();
    for attempt in 1..=3 {
        let claimed = app.claim(&["summary"]).await;
        assert_eq!(claimed["id"], json!(job.id), "attempt {attempt}");
        sqlx::query("UPDATE jobs SET heartbeat_at = now() - interval '1 hour' WHERE id = $1").bind(job.id).execute(&app.db).await.unwrap();
    }
    let r = reqwest::Client::new().post(app.url("/internal/jobs/claim")).header("x-worker-token", common::WORKER_TOKEN).json(&json!({"kinds": ["summary"]})).send().await.unwrap();
    assert_eq!(r.status(), 204, "not handed out a fourth time");
    let (state, _) = jobs::get(&app.db, job.id).await.unwrap().unwrap();
    assert!(state.failed() && state.error.unwrap().contains("repeatedly"));
}

// ---- names and addresses ----

#[tokio::test]
async fn renaming_keeps_projects_and_retires_the_old_name() {
    let app = common::TestApp::spawn().await;
    let ana = app.user("ana").await;
    app.project(&ana, "ana", "house", "private").await;
    assert_eq!(ana.patch(app.url("/api/me")).json(&json!({"slug": "ana-garcia"})).send().await.unwrap().status(), 200);
    let me: Value = ana.get(app.url("/api/me")).send().await.unwrap().json().await.unwrap();
    assert_eq!(me["slug"], "ana-garcia");
    assert_eq!(ana.get(app.url("/api/projects/ana-garcia/house")).send().await.unwrap().status(), 200);
    assert_eq!(ana.get(app.url("/api/projects/ana/house")).send().await.unwrap().status(), 404);
    // Nobody else can take the old name, as a user name or an organisation.
    let bob = app.user("bob").await;
    assert_eq!(bob.patch(app.url("/api/me")).json(&json!({"slug": "ana"})).send().await.unwrap().status(), 409);
    assert_eq!(bob.post(app.url("/api/orgs")).json(&json!({"slug": "ana"})).send().await.unwrap().status(), 409);
    // Its owner can take it back.
    assert_eq!(ana.patch(app.url("/api/me")).json(&json!({"slug": "ana"})).send().await.unwrap().status(), 200);
    assert_eq!(ana.get(app.url("/api/projects/ana/house")).send().await.unwrap().status(), 200);
}

#[tokio::test]
async fn a_rename_needs_a_valid_free_name_and_cannot_be_repeated_forever() {
    let app = common::TestApp::spawn().await;
    let ana = app.user("ana").await;
    app.user("bob").await;
    let rename = |slug: &'static str| ana.patch(app.url("/api/me")).json(&json!({"slug": slug})).send();
    assert_eq!(rename("Not Valid").await.unwrap().status(), 400);
    assert_eq!(rename("api").await.unwrap().status(), 400);
    assert_eq!(rename("bob").await.unwrap().status(), 409);
    assert_eq!(rename("ana").await.unwrap().status(), 200, "the same name is no change");
    for slug in ["ana-1", "ana-2", "ana-3"] {
        assert_eq!(rename(slug).await.unwrap().status(), 200, "{slug}");
    }
    assert_eq!(rename("ana-4").await.unwrap().status(), 403);
}

#[tokio::test]
async fn commits_made_for_a_person_never_carry_their_address() {
    let app = common::TestApp::spawn().await;
    let alice = app.user("alice").await;
    let r = alice.post(app.url("/api/projects/import")).multipart(import_form("alice", "model", b"ISO-10303-21;".to_vec())).send().await.unwrap();
    assert_eq!(r.status(), 200);
    let args: Value = sqlx::query_scalar("SELECT args FROM jobs WHERE kind = 'import_ifc'").fetch_one(&app.db).await.unwrap();
    let author = args["author"].as_array().unwrap();
    assert_eq!(author[0], "alice");
    let address = author[1].as_str().unwrap();
    assert!(!address.contains("example.com") && address.ends_with("@users.noreply.nicgit"), "{address}");
}

// ---- the workers' API ----

#[tokio::test]
async fn the_public_address_has_no_worker_api_and_the_workers_address_has_nothing_else() {
    let app = common::TestApp::spawn().await;
    let name = app.cfg.database_url.rsplit('/').next().unwrap().to_string();
    let cfg = Config::for_tests(&format!("postgres://nicgit:nicgit@127.0.0.1:5434/{name}"), app.data.path(), common::WORKER_TOKEN);
    let (routers, _) = app::build(Arc::new(cfg)).await.unwrap();
    let serve = |router: axum::Router| async move {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        format!("http://{addr}")
    };
    let public = serve(routers.public).await;
    let internal = serve(routers.internal).await;
    let c = reqwest::Client::new();
    let claim = |base: &str| c.post(format!("{base}/internal/jobs/claim")).header("x-worker-token", common::WORKER_TOKEN).json(&json!({"kinds": ["summary"]})).send();
    assert_eq!(claim(&internal).await.unwrap().status(), 204, "workers can claim here");
    assert_eq!(claim(&public).await.unwrap().status(), 404, "and only here");
    let id = Uuid::new_v4();
    for path in [format!("/internal/jobs/{id}/finish"), format!("/internal/jobs/{id}/progress")] {
        assert_eq!(c.post(format!("{public}{path}")).send().await.unwrap().status(), 404, "{path}");
    }
    assert_eq!(c.get(format!("{public}/internal/jobs/{id}/git/info/refs?service=git-upload-pack")).send().await.unwrap().status(), 404);
    assert_eq!(c.get(format!("{public}/internal/jobs/{id}/upload")).send().await.unwrap().status(), 404);
    assert_eq!(c.get(format!("{public}/api/health")).send().await.unwrap().status(), 200);
    assert_eq!(c.get(format!("{internal}/api/health")).send().await.unwrap().status(), 404);
    assert_eq!(c.get(format!("{internal}/api/me")).send().await.unwrap().status(), 404);
}
