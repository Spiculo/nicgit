// SPDX-License-Identifier: AGPL-3.0-or-later
//! What one engine job can reach (decision N30): its own project, through its own token, and nothing else.
mod common;
use common::git::{git, git_ok, kimbim_repo, write_record};
use nicgit_server::jobs;
use serde_json::{json, Value};
use std::path::Path;
use uuid::Uuid;

struct Fixture {
    app: common::TestApp,
    pid: Uuid,
    repo: tempfile::TempDir,
    alice: reqwest::Client,
}

/// alice/house with one version on main and a second option `other`.
async fn fixture() -> Fixture {
    fixture_with(|_| {}).await
}

async fn fixture_with(tweak: impl FnOnce(&mut nicgit_server::config::Config)) -> Fixture {
    let app = common::TestApp::spawn_with(tweak).await;
    let alice = app.user("alice").await;
    let p = app.project(&alice, "alice", "house", "private").await;
    let pid = Uuid::parse_str(p["id"].as_str().unwrap()).unwrap();
    let repo = kimbim_repo().await;
    git_ok(repo.path(), &["branch", "other"]).await;
    let token = app.token(&alice).await;
    let url = format!("http://alice:{token}@{}/alice/house.git", app.base.trim_start_matches("http://"));
    git_ok(repo.path(), &["push", "-q", &url, "main", "other"]).await;
    Fixture { app, pid, repo, alice }
}

impl Fixture {
    async fn job(&self, kind: &str, args: Value) -> Value {
        // Pushes queue summaries; finish them off so each test claims exactly the job it made.
        sqlx::query("UPDATE jobs SET status = 'done' WHERE status = 'queued'").execute(&self.app.db).await.unwrap();
        jobs::enqueue(&self.app.db, self.pid, kind, args, None).await.unwrap();
        self.app.claim(&[kind]).await
    }

    fn git_url(&self, job: &Value) -> String {
        self.app.url(&format!("/internal/jobs/{}/git", job["id"].as_str().unwrap()))
    }
}

fn header(job: &Value) -> String {
    format!("http.extraHeader=Authorization: Bearer {}", job["token"].as_str().unwrap())
}

fn job_id(job: &Value) -> &str {
    job["id"].as_str().unwrap()
}

/// A local clone through the job's own git route.
async fn clone(f: &Fixture, job: &Value) -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    let h = header(job);
    git_ok(d.path(), &["-c", &h, "clone", "-q", &f.git_url(job), "."]).await;
    git_ok(d.path(), &["config", "http.extraHeader", h.trim_start_matches("http.extraHeader=")]).await;
    d
}

fn head_sha(p: &Path) -> impl std::future::Future<Output = String> + '_ {
    git_ok(p, &["rev-parse", "HEAD"])
}

#[tokio::test]
async fn claim_returns_token_and_no_paths() {
    let f = fixture().await;
    let job = f.job("compare", json!({"from": "a", "to": "b"})).await;
    assert!(job["token"].as_str().unwrap().len() >= 32);
    for leak in ["repo", "cache_dir", "uploads_dir"] {
        assert!(job.get(leak).is_none(), "{leak} must not be sent to a worker");
    }
    let stored: String = sqlx::query_scalar("SELECT token_hash FROM jobs WHERE id = $1::uuid").bind(job_id(&job)).fetch_one(&f.app.db).await.unwrap();
    assert_ne!(stored, job["token"].as_str().unwrap(), "only a hash is stored");
}

#[tokio::test]
async fn pool_token_cannot_finish_or_fetch() {
    let f = fixture().await;
    let job = f.job("compare", json!({})).await;
    let c = reqwest::Client::new();
    let id = job_id(&job);
    for path in [format!("/internal/jobs/{id}/progress"), format!("/internal/jobs/{id}/finish")] {
        let r = c.post(f.app.url(&path)).header("x-worker-token", common::WORKER_TOKEN).json(&json!({"progress": 0.5, "result": {}})).send().await.unwrap();
        assert_eq!(r.status(), 401, "{path}");
        let r = c.post(f.app.url(&path)).bearer_auth(common::WORKER_TOKEN).json(&json!({"progress": 0.5, "result": {}})).send().await.unwrap();
        assert_eq!(r.status(), 401, "{path}");
    }
    let r = c.get(f.app.url(&format!("/internal/jobs/{id}/git/info/refs?service=git-upload-pack"))).bearer_auth(common::WORKER_TOKEN).send().await.unwrap();
    assert_eq!(r.status(), 401);
}

#[tokio::test]
async fn token_refused_on_other_job() {
    let f = fixture().await;
    let a = f.job("compare", json!({})).await;
    let b = f.job("settings_diff", json!({})).await;
    let c = reqwest::Client::new();
    let bid = job_id(&b);
    let t = a["token"].as_str().unwrap();
    assert_eq!(c.post(f.app.url(&format!("/internal/jobs/{bid}/finish"))).bearer_auth(t).json(&json!({"result": {}})).send().await.unwrap().status(), 401);
    assert_eq!(c.post(f.app.url(&format!("/internal/jobs/{bid}/progress"))).bearer_auth(t).json(&json!({"progress": 1})).send().await.unwrap().status(), 401);
    let r = c.get(f.app.url(&format!("/internal/jobs/{bid}/git/info/refs?service=git-upload-pack"))).bearer_auth(t).send().await.unwrap();
    assert_eq!(r.status(), 401);
}

#[tokio::test]
async fn token_dead_after_finish() {
    let f = fixture().await;
    let job = f.job("compare", json!({})).await;
    assert_eq!(f.app.finish(&job, json!({"result": {}})).await, 200);
    assert_eq!(f.app.finish(&job, json!({"result": {"tampered": true}})).await, 401);
    let r = reqwest::Client::new()
        .get(f.git_url(&job) + "/info/refs?service=git-upload-pack")
        .bearer_auth(job["token"].as_str().unwrap())
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 401);
    let stored: Option<String> = sqlx::query_scalar("SELECT token_hash FROM jobs WHERE id = $1::uuid").bind(job_id(&job)).fetch_one(&f.app.db).await.unwrap();
    assert!(stored.is_none());
}

#[tokio::test]
async fn token_dead_after_reclaim() {
    let f = fixture().await;
    let first = f.job("compare", json!({})).await;
    sqlx::query("UPDATE jobs SET heartbeat_at = now() - interval '20 minutes' WHERE id = $1::uuid").bind(job_id(&first)).execute(&f.app.db).await.unwrap();
    let second = f.app.claim(&["compare"]).await;
    assert_eq!(job_id(&first), job_id(&second));
    assert_ne!(first["token"], second["token"]);
    assert_eq!(f.app.finish(&first, json!({"result": {"from": "the lost worker"}})).await, 401);
    assert_eq!(f.app.finish(&second, json!({"result": {}})).await, 200);
}

#[test]
fn fetch_plan_by_kind() {
    let a = "a".repeat(40);
    let b = "b".repeat(40);
    let commits = |list: Value, depth: u64| json!({"mode": "commits", "commits": list, "depth": depth});
    let full = json!({"mode": "full"});
    assert_eq!(jobs::fetch_plan("export_ifc", &json!({"rev": a})), commits(json!([a]), 1));
    assert_eq!(jobs::fetch_plan("checks", &json!({"rev": a})), commits(json!([a]), 1));
    assert_eq!(jobs::fetch_plan("summary", &json!({"rev": a})), commits(json!([a]), 2));
    assert_eq!(jobs::fetch_plan("compare", &json!({"from": a, "to": b})), commits(json!([a, b]), 1));
    assert_eq!(jobs::fetch_plan("settings_diff", &json!({"from": a, "to": b})), commits(json!([a, b]), 1));
    assert_eq!(jobs::fetch_plan("compare", &json!({"from": a, "to": a})), commits(json!([a]), 1));
    for kind in ["merge_preview", "combine", "element_history", "export_kim", "import_ifc"] {
        assert_eq!(jobs::fetch_plan(kind, &json!({"rev": a})), full, "{kind}");
    }
    assert_eq!(jobs::fetch_plan("import_kim", &json!({})), json!({"mode": "none"}));
    // A name rather than a commit id: fall back to everything rather than fail the job.
    assert_eq!(jobs::fetch_plan("export_ifc", &json!({"rev": "main~1"})), full);
    assert_eq!(jobs::fetch_plan("compare", &json!({"from": a, "to": "main"})), full);
}

#[tokio::test]
async fn claim_carries_the_fetch_plan() {
    let f = fixture().await;
    let sha = head_sha(f.repo.path()).await;
    let job = f.job("export_ifc", json!({"rev": sha})).await;
    assert_eq!(job["fetch"], json!({"mode": "commits", "commits": [sha], "depth": 1}));
}

#[tokio::test]
async fn git_fetch_is_limited_to_job_project() {
    let f = fixture().await;
    let job = f.job("merge_preview", json!({})).await;
    let d = clone(&f, &job).await;
    assert_eq!(head_sha(d.path()).await, head_sha(f.repo.path()).await);
    // Someone else's project: the job's token means nothing on the ordinary git routes.
    let bob = f.app.user("bob").await;
    f.app.project(&bob, "bob", "secret", "private").await;
    let c = reqwest::Client::new();
    for path in ["/bob/secret.git/info/refs?service=git-upload-pack", "/alice/house.git/info/refs?service=git-upload-pack"] {
        let r = c.get(f.app.url(path)).bearer_auth(job["token"].as_str().unwrap()).send().await.unwrap();
        assert_eq!(r.status(), 401, "{path}");
    }
    let r = c.get(f.app.url("/api/projects/bob/secret")).bearer_auth(job["token"].as_str().unwrap()).send().await.unwrap();
    assert_eq!(r.status(), 404);
}

#[tokio::test]
async fn a_single_old_version_can_be_fetched_alone() {
    let f = fixture().await;
    let first = head_sha(f.repo.path()).await;
    write_record(f.repo.path(), "settings.json", "{\"units\":\"m\"}");
    git_ok(f.repo.path(), &["commit", "-qam", "second"]).await;
    let token = f.app.token(&f.alice).await;
    let url = format!("http://alice:{token}@{}/alice/house.git", f.app.base.trim_start_matches("http://"));
    git_ok(f.repo.path(), &["push", "-q", &url, "main"]).await;
    let job = f.job("export_ifc", json!({"rev": first})).await;
    let d = tempfile::tempdir().unwrap();
    git_ok(d.path(), &["init", "-q", "--bare", "."]).await;
    git_ok(d.path(), &["-c", &header(&job), "fetch", "-q", "--depth", "1", &f.git_url(&job), &first]).await;
    assert_eq!(git_ok(d.path(), &["rev-parse", "FETCH_HEAD"]).await, first);
    let count = git_ok(d.path(), &["rev-list", "--count", "FETCH_HEAD"]).await;
    assert_eq!(count, "1", "only the one version, no history");
}

#[tokio::test]
async fn readonly_job_cannot_push() {
    let f = fixture().await;
    let job = f.job("compare", json!({})).await;
    let c = reqwest::Client::new();
    let t = job["token"].as_str().unwrap();
    let r = c.get(f.git_url(&job) + "/info/refs?service=git-receive-pack").bearer_auth(t).send().await.unwrap();
    assert_eq!(r.status(), 403);
    let r = c.post(f.git_url(&job) + "/git-receive-pack").bearer_auth(t).body("0000").send().await.unwrap();
    assert_eq!(r.status(), 403);
}

#[tokio::test]
async fn combine_pushes_only_its_target_fast_forward() {
    let f = fixture().await;
    let job = f.job("combine", json!({"target": "main"})).await;
    let d = clone(&f, &job).await;
    write_record(d.path(), "settings.json", "{\"units\":\"cm\"}");
    git_ok(d.path(), &["commit", "-qam", "merged"]).await;

    // Another option: refused.
    assert!(!git(d.path(), &["push", "-q", "origin", "HEAD:refs/heads/other"]).await.status.success());
    // A new option or a milestone: refused.
    assert!(!git(d.path(), &["push", "-q", "origin", "HEAD:refs/heads/brand-new"]).await.status.success());
    assert!(!git(d.path(), &["push", "-q", "origin", "HEAD:refs/tags/v1"]).await.status.success());
    // Its target, fast-forward: accepted.
    git_ok(d.path(), &["push", "-q", "origin", "HEAD:refs/heads/main"]).await;
    // Rewriting the target: refused.
    git_ok(d.path(), &["commit", "-q", "--amend", "-m", "rewritten"]).await;
    assert!(!git(d.path(), &["push", "-qf", "origin", "HEAD:refs/heads/main"]).await.status.success());
    // Deleting the target: refused.
    assert!(!git(d.path(), &["push", "-q", "origin", ":refs/heads/main"]).await.status.success());
}

#[tokio::test]
async fn job_push_does_not_count_as_a_user_push() {
    let f = fixture().await;
    let job = f.job("combine", json!({"target": "main"})).await;
    let before: i64 = sqlx::query_scalar("SELECT count(*) FROM activity WHERE kind = 'push'").fetch_one(&f.app.db).await.unwrap();
    let d = clone(&f, &job).await;
    write_record(d.path(), "settings.json", "{\"units\":\"cm\"}");
    git_ok(d.path(), &["commit", "-qam", "merged"]).await;
    git_ok(d.path(), &["push", "-q", "origin", "HEAD:refs/heads/main"]).await;
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    let after: i64 = sqlx::query_scalar("SELECT count(*) FROM activity WHERE kind = 'push'").fetch_one(&f.app.db).await.unwrap();
    assert_eq!(before, after, "Combine records its own activity once it finishes");
}

#[tokio::test]
async fn import_cannot_overwrite_existing_ref() {
    let f = fixture().await;
    let job = f.job("import_kim", json!({})).await;
    let d = clone(&f, &job).await;
    write_record(d.path(), "settings.json", "{\"units\":\"cm\"}");
    git_ok(d.path(), &["commit", "-qam", "imported"]).await;
    assert!(!git(d.path(), &["push", "-q", "origin", "HEAD:refs/heads/main"]).await.status.success());
    assert!(!git(d.path(), &["push", "-q", "origin", ":refs/heads/other"]).await.status.success());
    git_ok(d.path(), &["push", "-q", "origin", "HEAD:refs/heads/imported", "HEAD:refs/tags/m1"]).await;
    assert!(!git(d.path(), &["push", "-q", "origin", "HEAD:refs/for/review"]).await.status.success());
}

#[tokio::test]
async fn invalid_record_push_refused() {
    let f = fixture().await;
    let job = f.job("combine", json!({"target": "main"})).await;
    let d = clone(&f, &job).await;
    write_record(d.path(), "objects/ab/0Zukam4n1BpBYDaJ2UpTma.json", "<<<<<<< a text merge");
    git_ok(d.path(), &["commit", "-qam", "broken"]).await;
    let out = git(d.path(), &["push", "origin", "HEAD:refs/heads/main"]).await;
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("not valid KimBim JSON"));
}

#[tokio::test]
async fn artifact_round_trip_serves_the_model() {
    let f = fixture().await;
    let sha = head_sha(f.repo.path()).await;
    sqlx::query("UPDATE jobs SET status = 'done' WHERE status = 'queued'").execute(&f.app.db).await.unwrap();
    // Asking for the model queues the export.
    let r = f.alice.get(f.app.url(&format!("/api/projects/alice/house/versions/{sha}/model.ifc"))).send().await.unwrap();
    assert_eq!(r.status(), 202);
    let job = f.app.claim(&["export_ifc"]).await;
    let name = format!("ifc/{sha}.ifc");
    let c = reqwest::Client::new();
    let put = c.put(f.app.url(&format!("/internal/jobs/{}/artifacts/{name}", job_id(&job)))).bearer_auth(job["token"].as_str().unwrap()).body("ISO-10303-21;").send().await.unwrap();
    assert_eq!(put.status(), 200);
    assert_eq!(f.app.finish(&job, json!({"result": {"artifact": name, "commit": sha}})).await, 200);
    let r = f.alice.get(f.app.url(&format!("/api/projects/alice/house/versions/{sha}/model.ifc"))).send().await.unwrap();
    assert_eq!(r.status(), 200);
    assert_eq!(r.text().await.unwrap(), "ISO-10303-21;");

    // A checks job on the same project can read it back; an export cannot.
    let checks = f.job("checks", json!({"rev": sha, "checks": []})).await;
    let url = f.app.url(&format!("/internal/jobs/{}/artifacts/{name}", job_id(&checks)));
    let got = c.get(&url).bearer_auth(checks["token"].as_str().unwrap()).send().await.unwrap();
    assert_eq!(got.status(), 200);
    assert_eq!(got.text().await.unwrap(), "ISO-10303-21;");
    let missing = c.get(f.app.url(&format!("/internal/jobs/{}/artifacts/ifc/{}.ifc", job_id(&checks), "f".repeat(40)))).bearer_auth(checks["token"].as_str().unwrap()).send().await.unwrap();
    assert_eq!(missing.status(), 404);
}

#[tokio::test]
async fn result_naming_a_path_outside_the_cache_is_not_served() {
    let f = fixture().await;
    let sha = head_sha(f.repo.path()).await;
    sqlx::query("UPDATE jobs SET status = 'done' WHERE status = 'queued'").execute(&f.app.db).await.unwrap();
    f.alice.get(f.app.url(&format!("/api/projects/alice/house/versions/{sha}/model.ifc"))).send().await.unwrap();
    let job = f.app.claim(&["export_ifc"]).await;
    let secret = f.app.data.path().join("secret.ifc");
    std::fs::write(&secret, "another customer's model").unwrap();
    for artifact in [json!("../../secret.ifc"), json!(secret.to_string_lossy()), json!(null)] {
        sqlx::query("UPDATE jobs SET status = 'done', result = $2 WHERE id = $1::uuid").bind(job_id(&job)).bind(json!({"artifact": artifact, "path": secret.to_string_lossy()})).execute(&f.app.db).await.unwrap();
        let r = f.alice.get(f.app.url(&format!("/api/projects/alice/house/versions/{sha}/model.ifc"))).send().await.unwrap();
        assert_eq!(r.status(), 202, "{artifact}: export again instead");
    }
}

#[tokio::test]
async fn artifact_paths_are_refused() {
    let f = fixture().await;
    let job = f.job("export_ifc", json!({})).await;
    let c = reqwest::Client::new();
    let id = job_id(&job);
    for path in ["..%2f..%2fescape.ifc", "ifc%2f..%2f..%2fescape.ifc", "ifc/..%2fescape.ifc", "ifc/.hidden.ifc", "ifc/a/b.ifc", "exports/a.ifc", "ifc/a.exe", "escape.ifc"] {
        let r = c.put(f.app.url(&format!("/internal/jobs/{id}/artifacts/{path}"))).bearer_auth(job["token"].as_str().unwrap()).body("x").send().await.unwrap();
        assert!(r.status() == 400 || r.status() == 403, "{path}: {}", r.status());
    }
    assert!(!f.app.data.path().join("escape.ifc").exists());
    assert!(!f.app.data.path().join("cache").join("escape.ifc").exists());
    assert!(!f.app.data.path().join("cache").join(f.pid.to_string()).join("escape.ifc").exists());
}

#[tokio::test]
async fn artifact_kind_not_allowed() {
    let f = fixture().await;
    let job = f.job("compare", json!({})).await;
    let c = reqwest::Client::new();
    let url = f.app.url(&format!("/internal/jobs/{}/artifacts/ifc/a.ifc", job_id(&job)));
    let t = job["token"].as_str().unwrap();
    assert_eq!(c.put(&url).bearer_auth(t).body("x").send().await.unwrap().status(), 403);
    assert_eq!(c.get(&url).bearer_auth(t).send().await.unwrap().status(), 403);
    assert_eq!(c.get(&url).header("x-worker-token", common::WORKER_TOKEN).send().await.unwrap().status(), 401);
}

#[tokio::test]
async fn artifact_size_limit() {
    let f = fixture_with(|cfg| cfg.max_repo_mb = 1).await;
    let job = f.job("export_ifc", json!({})).await;
    let r = reqwest::Client::new()
        .put(f.app.url(&format!("/internal/jobs/{}/artifacts/ifc/a.ifc", job_id(&job))))
        .bearer_auth(job["token"].as_str().unwrap())
        .body(vec![0u8; (1 << 20) + 1])
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 413);
    let dir = f.app.data.path().join("cache").join(f.pid.to_string()).join("ifc");
    let left = std::fs::read_dir(&dir).map(|d| d.count()).unwrap_or(0);
    assert_eq!(left, 0, "nothing is left behind, not even a partial file");
}

async fn import_ifc(f: &Fixture, slug: &str) -> Uuid {
    let form = reqwest::multipart::Form::new()
        .text("owner", "alice")
        .text("slug", slug.to_string())
        .part("file", reqwest::multipart::Part::bytes(b"ISO-10303-21;".to_vec()).file_name("model.ifc"));
    let r = f.alice.post(f.app.url("/api/projects/import")).multipart(form).send().await.unwrap();
    assert_eq!(r.status(), 200);
    let v: Value = r.json().await.unwrap();
    Uuid::parse_str(v["project"]["id"].as_str().unwrap()).unwrap()
}

#[tokio::test]
async fn upload_stored_per_project_served_once_and_removed_on_finish() {
    let f = fixture().await;
    sqlx::query("UPDATE jobs SET status = 'done' WHERE status = 'queued'").execute(&f.app.db).await.unwrap();
    let project = import_ifc(&f, "imported").await;
    let job = f.app.claim(&["import_ifc"]).await;
    let name = job["args"]["upload"].as_str().unwrap().to_string();
    let path = f.app.data.path().join("uploads").join(project.to_string()).join(&name);
    assert!(path.is_file(), "stored under the project");
    let c = reqwest::Client::new();
    let r = c.get(f.app.url(&format!("/internal/jobs/{}/upload", job_id(&job)))).bearer_auth(job["token"].as_str().unwrap()).send().await.unwrap();
    assert_eq!(r.status(), 200);
    assert_eq!(r.bytes().await.unwrap().as_ref(), b"ISO-10303-21;");
    assert_eq!(f.app.finish(&job, json!({"error": "bad file"})).await, 200);
    assert!(!path.exists(), "gone once the job is finished");
    assert!(!f.app.data.path().join("uploads").join("incoming").join(&name).exists());
}

#[tokio::test]
async fn upload_only_for_import_jobs() {
    let f = fixture().await;
    let job = f.job("compare", json!({"upload": "../../secrets"})).await;
    let r = reqwest::Client::new().get(f.app.url(&format!("/internal/jobs/{}/upload", job_id(&job)))).bearer_auth(job["token"].as_str().unwrap()).send().await.unwrap();
    assert_eq!(r.status(), 403);
}

#[tokio::test]
async fn upload_of_another_project_is_not_reachable() {
    let f = fixture().await;
    sqlx::query("UPDATE jobs SET status = 'done' WHERE status = 'queued'").execute(&f.app.db).await.unwrap();
    let other = import_ifc(&f, "other-import").await;
    let theirs = f.app.claim(&["import_ifc"]).await;
    let name = theirs["args"]["upload"].as_str().unwrap().to_string();
    assert!(f.app.data.path().join("uploads").join(other.to_string()).join(&name).is_file());
    // A job on another project, naming the same file.
    for args in [json!({"upload": name}), json!({"upload": format!("../{other}/{name}")})] {
        let job = f.job("import_ifc", args).await;
        let r = reqwest::Client::new().get(f.app.url(&format!("/internal/jobs/{}/upload", job_id(&job)))).bearer_auth(job["token"].as_str().unwrap()).send().await.unwrap();
        assert!(r.status() == 404 || r.status() == 400, "{}", r.status());
        f.app.finish(&job, json!({"error": "x"})).await;
    }
    assert!(f.app.data.path().join("uploads").join(other.to_string()).join(&name).is_file(), "and cannot delete it either");
}

#[tokio::test]
async fn combine_result_must_match_target_head() {
    let f = fixture().await;
    let real = head_sha(f.repo.path()).await;
    let job = f.job("combine", json!({"target": "main"})).await;
    assert_eq!(f.app.finish(&job, json!({"result": {"commit": "f".repeat(40), "status": "merge"}})).await, 200);
    let (status, error): (String, Option<String>) = sqlx::query_as("SELECT status, error FROM jobs WHERE id = $1::uuid").bind(job_id(&job)).fetch_one(&f.app.db).await.unwrap();
    assert_eq!(status, "failed");
    assert!(error.unwrap().contains("does not match"));

    let job = f.job("combine", json!({"target": "main"})).await;
    assert_eq!(f.app.finish(&job, json!({"result": {"commit": real, "status": "up_to_date"}})).await, 200);
    let status: String = sqlx::query_scalar("SELECT status FROM jobs WHERE id = $1::uuid").bind(job_id(&job)).fetch_one(&f.app.db).await.unwrap();
    assert_eq!(status, "done");
}

#[tokio::test]
async fn import_default_option_must_exist() {
    let f = fixture().await;
    let job = f.job("import_kim", json!({})).await;
    assert_eq!(f.app.finish(&job, json!({"result": {"default": "does-not-exist", "options": [], "heads": {}}})).await, 200);
    let default: String = sqlx::query_scalar("SELECT default_option FROM projects WHERE id = $1").bind(f.pid).fetch_one(&f.app.db).await.unwrap();
    assert_eq!(default, "main");

    let job = f.job("import_kim", json!({})).await;
    assert_eq!(f.app.finish(&job, json!({"result": {"default": "other", "options": ["other"], "heads": {}}})).await, 200);
    let default: String = sqlx::query_scalar("SELECT default_option FROM projects WHERE id = $1").bind(f.pid).fetch_one(&f.app.db).await.unwrap();
    assert_eq!(default, "other");
    let head = std::fs::read_to_string(f.app.data.path().join("repos").join(f.pid.to_string()).join(".git/HEAD")).unwrap();
    assert_eq!(head.trim(), "ref: refs/heads/other");
}
