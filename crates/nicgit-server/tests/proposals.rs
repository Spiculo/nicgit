// SPDX-License-Identifier: AGPL-3.0-or-later
mod common;
use common::git::{git_ok, kimbim_repo, write_record};
use common::engine;
use serde_json::{json, Value};

struct Fixture {
    app: common::TestApp,
    alice: reqwest::Client,
    bob: reqwest::Client,
    carol: reqwest::Client,
    src: tempfile::TempDir,
    url: String,
}

/// alice (owner) with bob (member) and carol (reviewer); options `main` and `scheme-b`.
async fn fixture() -> Fixture {
    let app = common::TestApp::spawn().await;
    let alice = app.user("alice").await;
    let bob = app.user("bob").await;
    let carol = app.user("carol").await;
    let at = app.token(&alice).await;
    app.project(&alice, "alice", "house", "private").await;
    for (who, role) in [("bob", "member"), ("carol", "reviewer")] {
        alice.put(app.url(&format!("/api/projects/alice/house/members/{who}"))).json(&json!({"role": role})).send().await.unwrap();
    }
    let src = kimbim_repo().await;
    git_ok(src.path(), &["checkout", "-qb", "scheme-b"]).await;
    write_record(src.path(), "objects/cd/1111111111111111111111.json", "{\"a\":{\"Name\":\"W-2\"},\"t\":\"IfcWall\"}");
    git_ok(src.path(), &["add", "-A"]).await;
    git_ok(src.path(), &["commit", "-qm", "Scheme B: second wall"]).await;
    let url = format!("http://git:{at}@{}/alice/house.git", app.base.trim_start_matches("http://"));
    git_ok(src.path(), &["push", &url, "main", "scheme-b"]).await;
    Fixture { app, alice, bob, carol, src, url }
}

impl Fixture {
    fn u(&self, p: &str) -> String {
        self.app.url(&format!("/api/projects/alice/house{p}"))
    }
    async fn open(&self, c: &reqwest::Client) -> Value {
        let r = c
            .post(self.u("/proposals"))
            .json(&json!({"title": "Scheme B", "body": "Adds a wall", "source": "scheme-b", "target": "main"}))
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 200, "{}", r.text().await.unwrap());
        r.json().await.unwrap()
    }
    async fn protect_main(&self, body: Value) {
        let r = self.alice.put(self.u("/rules/main")).json(&body).send().await.unwrap();
        assert_eq!(r.status(), 200, "{}", r.text().await.unwrap());
    }
    async fn review(&self, c: &reqwest::Client, verdict: &str) -> reqwest::StatusCode {
        c.post(self.u("/proposals/1/reviews")).json(&json!({"verdict": verdict, "body": ""})).send().await.unwrap().status()
    }
    async fn detail(&self) -> Value {
        self.alice.get(self.u("/proposals/1")).send().await.unwrap().json().await.unwrap()
    }
}

#[tokio::test]
async fn open_proposal_and_list() {
    let f = fixture().await;
    let p = f.open(&f.bob).await;
    assert_eq!(p["number"], 1);
    assert_eq!(p["state"], "open");
    let list: Value = f.carol.get(f.u("/proposals?state=open")).send().await.unwrap().json().await.unwrap();
    assert_eq!(list.as_array().unwrap().len(), 1);
    assert_eq!(list[0]["title"], "Scheme B");
    // The proposal's head is kept as a ref, so it survives the option being deleted.
    let refs = git_ok(f.src.path(), &["ls-remote", &f.url]).await;
    assert!(refs.contains("refs/proposals/1/head"), "{refs}");
    // Reviewers cannot open proposals; an option cannot be combined into itself.
    let r = f.carol.post(f.u("/proposals")).json(&json!({"title": "x", "source": "scheme-b", "target": "main"})).send().await.unwrap();
    assert_eq!(r.status(), 403);
    let r = f.bob.post(f.u("/proposals")).json(&json!({"title": "x", "source": "main", "target": "main"})).send().await.unwrap();
    assert_eq!(r.status(), 400);
    let r = f.bob.post(f.u("/proposals")).json(&json!({"title": "x", "source": "nope", "target": "main"})).send().await.unwrap();
    assert_eq!(r.status(), 404);
}

#[tokio::test]
async fn approvals_counted_by_role() {
    let f = fixture().await;
    f.protect_main(json!({"required_approvals": 1, "required_roles": ["lead"]})).await;
    f.open(&f.bob).await;
    assert_eq!(f.review(&f.carol, "approve").await, 200);
    let d = f.detail().await;
    assert_eq!(d["approvals"]["ok"], false);
    assert_eq!(d["approvals"]["missing"][0]["kind"], "role");
    assert_eq!(f.review(&f.alice, "approve").await, 200);
    let d = f.detail().await;
    assert_eq!(d["approvals"]["ok"], true, "{d}");
    assert_eq!(d["reviews"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn author_approval_not_counted() {
    let f = fixture().await;
    f.protect_main(json!({"required_approvals": 1})).await;
    f.open(&f.bob).await;
    assert_eq!(f.review(&f.bob, "approve").await, 400);
}

#[tokio::test]
async fn new_push_dismisses_approvals() {
    let f = fixture().await;
    f.protect_main(json!({"required_approvals": 1, "dismiss_on_push": true})).await;
    f.open(&f.bob).await;
    f.review(&f.alice, "approve").await;
    assert_eq!(f.detail().await["approvals"]["ok"], true);
    write_record(f.src.path(), "objects/ef/2222222222222222222222.json", "{\"a\":{\"Name\":\"W-3\"},\"t\":\"IfcWall\"}");
    git_ok(f.src.path(), &["add", "-A"]).await;
    git_ok(f.src.path(), &["commit", "-qm", "Scheme B: third wall"]).await;
    git_ok(f.src.path(), &["push", &f.url, "scheme-b"]).await;
    let head = git_ok(f.src.path(), &["rev-parse", "scheme-b"]).await;
    for _ in 0..50 {
        let d = f.detail().await;
        if d["proposal"]["source_head"] == head.as_str() {
            assert_eq!(d["approvals"]["ok"], false, "{d}");
            assert_eq!(d["reviews"][0]["dismissed"], true);
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    panic!("the proposal never followed its option");
}

#[tokio::test]
async fn combine_blocked_without_approvals() {
    let f = fixture().await;
    f.protect_main(json!({"required_approvals": 1})).await;
    f.open(&f.bob).await;
    let r = f.alice.post(f.u("/proposals/1/combine")).json(&json!({"mode": "auto"})).send().await.unwrap();
    assert_eq!(r.status(), 409);
    let v: Value = r.json().await.unwrap();
    assert_eq!(v["error"], "approvals_missing");
}

#[tokio::test]
async fn member_cannot_combine_into_protected() {
    let f = fixture().await;
    f.protect_main(json!({"required_approvals": 0})).await;
    f.open(&f.bob).await;
    let r = f.bob.post(f.u("/proposals/1/combine")).json(&json!({"mode": "auto"})).send().await.unwrap();
    assert_eq!(r.status(), 403);
}

#[tokio::test]
async fn mode_not_allowed_rejected() {
    let f = fixture().await;
    f.protect_main(json!({"required_approvals": 0, "allowed_modes": ["granular"]})).await;
    f.open(&f.bob).await;
    let r = f.alice.post(f.u("/proposals/1/combine")).json(&json!({"mode": "whole:theirs"})).send().await.unwrap();
    assert_eq!(r.status(), 409);
    let v: Value = r.json().await.unwrap();
    assert_eq!(v["error"], "mode_not_allowed");
}

#[tokio::test]
async fn close_and_comment() {
    let f = fixture().await;
    f.open(&f.bob).await;
    let r = f
        .carol
        .post(f.u("/proposals/1/comments"))
        .json(&json!({"body": "Check this wall", "pin": {"elements": ["0Zukam4n1BpBYDaJ2UpTma"]}}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let d = f.detail().await;
    assert_eq!(d["comments"][0]["body"], "Check this wall");
    assert_eq!(d["comments"][0]["pin"]["elements"][0], "0Zukam4n1BpBYDaJ2UpTma");
    assert_eq!(f.carol.post(f.u("/proposals/1/close")).send().await.unwrap().status(), 403);
    assert_eq!(f.bob.post(f.u("/proposals/1/close")).send().await.unwrap().status(), 200);
    assert_eq!(f.detail().await["proposal"]["state"], "closed");
}

#[tokio::test]
async fn combine_whole_theirs_moves_target() {
    if !engine::enabled() {
        return eprintln!("skipped: set NICGIT_TEST_ENGINE=1");
    }
    let app = common::TestApp::spawn().await;
    let alice = engine::push_m5(&app).await;
    // Put main back before the combine so both options have their own versions.
    let fx = engine::m5_fixture().to_string_lossy().to_string();
    let before_main = git_ok(std::path::Path::new("."), &["--git-dir", &fx, "rev-parse", "main^1"]).await;
    let at = app.token(&alice).await;
    let url = format!("http://git:{at}@{}/alice/m5.git", app.base.trim_start_matches("http://"));
    git_ok(std::path::Path::new("."), &["--git-dir", &fx, "push", "--force", &url, &format!("{before_main}:refs/heads/main")]).await;
    let u = |p: &str| app.url(&format!("/api/projects/alice/m5{p}"));
    let r = alice
        .post(u("/proposals"))
        .json(&json!({"title": "Scheme B", "source": "Scheme%20B", "target": "main"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    // The worker runs while the API waits for the preview and the combine.
    let worker = {
        let base = app.base.clone();
        tokio::spawn(async move {
            let script = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scripts/engine-python.sh");
            tokio::process::Command::new("sh")
                .arg(script)
                // Killing `sh` leaves the engine's process behind on Windows, holding this test's pipes open.
                .args(["-m", "nicgit_worker", "--url", &base, "--idle", "0.2", "--exit-if-unreachable"])
                .env("NICGIT_WORKER_TOKEN", common::WORKER_TOKEN)
                .kill_on_drop(true)
                .output()
                .await
        })
    };
    let preview: Value = loop {
        let r = alice.get(u("/proposals/1/preview")).send().await.unwrap();
        if r.status() == 200 {
            break r.json().await.unwrap();
        }
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    };
    assert_eq!(preview["status"], "merge");
    assert!(!preview["conflicts"].as_array().unwrap().is_empty(), "{preview}");
    let r = alice.post(u("/proposals/1/combine")).json(&json!({"mode": "auto"})).send().await.unwrap();
    assert_eq!(r.status(), 409, "auto refuses conflicts");
    let r = alice.post(u("/proposals/1/combine")).json(&json!({"mode": "whole:theirs"})).send().await.unwrap();
    let status = r.status();
    let v: Value = r.json().await.unwrap();
    assert_eq!(status, 200, "{v}");
    let d: Value = alice.get(u("/proposals/1")).send().await.unwrap().json().await.unwrap();
    assert_eq!(d["proposal"]["state"], "combined");
    let main = git_ok(std::path::Path::new("."), &["ls-remote", &url, "refs/heads/main"]).await;
    assert!(main.starts_with(v["commit"].as_str().unwrap()), "{main} vs {v}");

    // Golden: Combine through the job's temporary copy and push gives exactly the engine's own merge,
    // made here on a private clone of the same two heads with the same choice.
    let tmp = tempfile::tempdir().unwrap();
    let store = tmp.path().join("p");
    git_ok(tmp.path(), &["clone", "--bare", "-q", &fx, &store.join(".git").to_string_lossy()]).await;
    git_ok(tmp.path(), &["--git-dir", &store.join(".git").to_string_lossy(), "update-ref", "refs/heads/main", &before_main]).await;
    let code = "import pathlib, sys, tempfile\n\
        from kimbim.project import Project\n\
        work = pathlib.Path(tempfile.mkdtemp())\n\
        (work / '.nicbim').mkdir()\n\
        (work / '.nicbim' / 'HEAD').write_text('ref: refs/heads/main\\n')\n\
        p = Project.open_worktree(pathlib.Path(sys.argv[1]), work)\n\
        preview = p.merge_preview('Scheme%20B')\n\
        picks = {c.to_dict()['id']: 'theirs' for c in preview.result.conflicts}\n\
        print(p.merge('Scheme%20B', 'golden', picks, author=('T', 't@example.com'), time=0))\n";
    let script = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scripts/engine-python.sh");
    let out = tokio::process::Command::new("sh").arg(script).args(["-c", code, &store.to_string_lossy()]).output().await.unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let engine_commit = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let tree_of = |git_dir: String, rev: String| async move { git_ok(std::path::Path::new("."), &["--git-dir", &git_dir, "rev-parse", &format!("{rev}^{{tree}}")]).await };
    let expected = tree_of(store.join(".git").to_string_lossy().to_string(), engine_commit).await;
    let id: uuid::Uuid = sqlx::query_scalar("SELECT id FROM projects WHERE slug = 'm5'").fetch_one(&app.db).await.unwrap();
    let server_git = app.cfg.data_dir.join("repos").join(id.to_string()).join(".git");
    let got = tree_of(server_git.to_string_lossy().to_string(), v["commit"].as_str().unwrap().to_string()).await;
    assert_eq!(got, expected, "the server's Combine and the engine's merge must give the same tree");
    worker.abort();
}
