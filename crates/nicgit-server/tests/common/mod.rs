// SPDX-License-Identifier: AGPL-3.0-or-later
//! Test harness: a server on a random port with its own database and data folder.
#![allow(dead_code)]
pub mod engine;
pub mod git;
use nicgit_server::{app, auth::SESSION_COOKIE, config::Config, dev};
use serde_json::{json, Value};
use sqlx::{Connection, Executor, PgConnection, PgPool};
use std::sync::Arc;

pub const ADMIN_URL: &str = "postgres://nicgit:nicgit@127.0.0.1:5434/nicgit";
pub const WORKER_TOKEN: &str = "test-worker-token";

/// Test databases are dropped on a later run: a finished or crashed test cannot clean up after itself.
/// Once per test process, and only those over an hour old, so parallel runs never lose theirs.
async fn sweep_old_databases(admin: &mut PgConnection, now: u64) {
    static DONE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if DONE.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    let names: Vec<String> = sqlx::query_scalar("SELECT datname FROM pg_database WHERE starts_with(datname, 'nicgit_test_')").fetch_all(&mut *admin).await.unwrap_or_default();
    for name in names {
        let stamp = name.strip_prefix("nicgit_test_").and_then(|r| r.split_once('_')).and_then(|(s, _)| s.parse::<u64>().ok());
        if stamp.is_none_or(|s| now.saturating_sub(s) > 3600) {
            let _ = admin.execute(format!("DROP DATABASE IF EXISTS {name} WITH (FORCE)").as_str()).await;
        }
    }
}

pub struct TestApp {
    pub base: String,
    pub client: reqwest::Client,
    pub db: PgPool,
    pub cfg: Arc<Config>,
    pub data: tempfile::TempDir,
}

impl TestApp {
    pub async fn spawn() -> TestApp {
        Self::spawn_with(|_| {}).await
    }

    pub async fn spawn_with(tweak: impl FnOnce(&mut Config)) -> TestApp {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
        let name = format!("nicgit_test_{stamp}_{}", uuid::Uuid::new_v4().simple());
        let mut admin = PgConnection::connect(ADMIN_URL).await.expect("dev Postgres on 5434 (deploy/compose.dev.yaml)");
        sweep_old_databases(&mut admin, stamp).await;
        admin.execute(format!("CREATE DATABASE {name}").as_str()).await.unwrap();
        let data = tempfile::tempdir().unwrap();
        let mut cfg = Config::for_tests(&format!("postgres://nicgit:nicgit@127.0.0.1:5434/{name}"), data.path(), WORKER_TOKEN);
        cfg.hook_exe = env!("CARGO_BIN_EXE_nicgit-server").into();
        tweak(&mut cfg);
        let cfg = Arc::new(cfg);
        let (routers, state) = app::build(cfg.clone()).await.unwrap();
        // One address for both, so tests reach everything; tests/limits.rs checks they really are separate.
        let router = routers.public.merge(routers.internal);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        let client = reqwest::Client::builder().cookie_store(true).build().unwrap();
        TestApp { base: format!("http://{addr}"), client, db: state.db.clone(), cfg, data }
    }

    pub fn url(&self, path: &str) -> String {
        format!("{}{}", self.base, path)
    }

    /// A fresh client (own cookie jar) with a new account `slug`, signed in.
    pub async fn user(&self, slug: &str) -> reqwest::Client {
        let id = dev::create_user(&self.db, slug, &format!("{slug}@example.com"), slug).await.expect("create user");
        let session = dev::new_session(&self.db, id).await.expect("new session");
        let jar = reqwest::cookie::Jar::default();
        jar.add_cookie_str(&format!("{SESSION_COOKIE}={session}"), &self.base.parse().unwrap());
        reqwest::Client::builder().cookie_provider(Arc::new(jar)).build().unwrap()
    }

    /// Claim one job as a worker's supervisor would; panics if none of `kinds` is waiting.
    pub async fn claim(&self, kinds: &[&str]) -> Value {
        let r = reqwest::Client::new().post(self.url("/internal/jobs/claim")).header("x-worker-token", WORKER_TOKEN).json(&json!({"kinds": kinds})).send().await.unwrap();
        assert_eq!(r.status(), 200, "no job of kind {kinds:?} waiting");
        r.json().await.unwrap()
    }

    /// Finish a claimed job with its own token.
    pub async fn finish(&self, job: &Value, body: Value) -> reqwest::StatusCode {
        reqwest::Client::new()
            .post(self.url(&format!("/internal/jobs/{}/finish", job["id"].as_str().unwrap())))
            .bearer_auth(job["token"].as_str().unwrap())
            .json(&body)
            .send()
            .await
            .unwrap()
            .status()
    }

    /// A personal access token for a logged-in client.
    pub async fn token(&self, c: &reqwest::Client) -> String {
        let v: Value = c.post(self.url("/api/me/tokens")).json(&json!({"name": "git"})).send().await.unwrap().json().await.unwrap();
        v["token"].as_str().unwrap().to_string()
    }

    pub async fn project(&self, c: &reqwest::Client, owner: &str, slug: &str, visibility: &str) -> Value {
        let r = c
            .post(self.url("/api/projects"))
            .json(&json!({"owner": owner, "slug": slug, "visibility": visibility}))
            .send()
            .await
            .unwrap();
        assert!(r.status().is_success(), "create project: {}", r.text().await.unwrap());
        r.json().await.unwrap()
    }
}
