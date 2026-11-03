// SPDX-License-Identifier: AGPL-3.0-or-later
//! Test harness: a server on a random port with its own database and data folder.
#![allow(dead_code)]
pub mod git;
use nicgit_server::{config::Config, app};
use serde_json::{json, Value};
use sqlx::{Connection, Executor, PgConnection, PgPool};
use std::sync::Arc;

pub const ADMIN_URL: &str = "postgres://nicgit:nicgit@127.0.0.1:5434/nicgit";
pub const WORKER_TOKEN: &str = "test-worker-token";

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
        let name = format!("nicgit_test_{}", uuid::Uuid::new_v4().simple());
        let mut admin = PgConnection::connect(ADMIN_URL).await.expect("dev Postgres on 5434 (deploy/compose.dev.yaml)");
        admin.execute(format!("CREATE DATABASE {name}").as_str()).await.unwrap();
        let data = tempfile::tempdir().unwrap();
        let mut cfg = Config::for_tests(&format!("postgres://nicgit:nicgit@127.0.0.1:5434/{name}"), data.path(), WORKER_TOKEN);
        cfg.hook_exe = env!("CARGO_BIN_EXE_nicgit-server").into();
        tweak(&mut cfg);
        let cfg = Arc::new(cfg);
        let (router, state) = app::build(cfg.clone()).await.unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        let client = reqwest::Client::builder().cookie_store(true).build().unwrap();
        TestApp { base: format!("http://{addr}"), client, db: state.db.clone(), cfg, data }
    }

    pub fn url(&self, path: &str) -> String {
        format!("{}{}", self.base, path)
    }

    /// A fresh client (own cookie jar) signed up and logged in as `slug`.
    pub async fn user(&self, slug: &str) -> reqwest::Client {
        let c = reqwest::Client::builder().cookie_store(true).build().unwrap();
        let r = c
            .post(self.url("/api/auth/signup"))
            .json(&json!({"slug": slug, "email": format!("{slug}@example.com"), "password": "correct horse battery"}))
            .send()
            .await
            .unwrap();
        assert!(r.status().is_success(), "signup {slug}: {}", r.text().await.unwrap());
        c
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
