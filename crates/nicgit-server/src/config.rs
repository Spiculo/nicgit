// SPDX-License-Identifier: AGPL-3.0-or-later
use std::path::{Path, PathBuf};

/// Server configuration, read from `NICGIT_*` environment variables.
#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub data_dir: PathBuf,
    pub bind: String,
    pub worker_token: String,
    pub public_url: String,
    pub ai_router_url: Option<String>,
    pub ai_router_key: Option<String>,
    pub max_upload_mb: u64,
    /// Folder with the built web app (`web/dist`); not served when missing.
    pub web_dir: Option<PathBuf>,
    /// The `git` executable.
    pub git: String,
    /// The `nicgit-server` executable that git hooks call back into.
    pub hook_exe: PathBuf,
    /// Google sign-in. Without these nobody can sign in, only use existing access tokens.
    pub google_client_id: Option<String>,
    pub google_client_secret: Option<String>,
    pub google_auth_url: String,
    pub google_token_url: String,
    /// The account with this verified Google email is the site admin (decision N28).
    pub site_admin_email: Option<String>,
}

pub const GOOGLE_AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
pub const GOOGLE_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
pub const DEFAULT_DATABASE_URL: &str = "postgres://nicgit:nicgit@127.0.0.1:5434/nicgit";

fn var(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

impl Config {
    pub fn from_env() -> anyhow::Result<Config> {
        let bind = var("NICGIT_BIND").unwrap_or_else(|| "127.0.0.1:3000".into());
        Ok(Config {
            database_url: Self::database_url_from_env(),
            data_dir: PathBuf::from(var("NICGIT_DATA_DIR").unwrap_or_else(|| "data".into())),
            public_url: var("NICGIT_PUBLIC_URL").unwrap_or_else(|| format!("http://{bind}")),
            bind,
            worker_token: var("NICGIT_WORKER_TOKEN")
                .ok_or_else(|| anyhow::anyhow!("NICGIT_WORKER_TOKEN must be set (shared with engine workers)"))?,
            ai_router_url: var("NICGIT_AI_ROUTER_URL"),
            ai_router_key: var("NICGIT_AI_ROUTER_KEY"),
            max_upload_mb: var("NICGIT_MAX_UPLOAD_MB").and_then(|v| v.parse().ok()).unwrap_or(2048),
            web_dir: var("NICGIT_WEB_DIR").map(PathBuf::from).or_else(|| {
                let d = PathBuf::from("web/dist");
                d.is_dir().then_some(d)
            }),
            git: var("NICGIT_GIT").unwrap_or_else(|| "git".into()),
            hook_exe: std::env::current_exe()?,
            google_client_id: var("NICGIT_GOOGLE_CLIENT_ID"),
            google_client_secret: var("NICGIT_GOOGLE_CLIENT_SECRET"),
            google_auth_url: GOOGLE_AUTH_URL.into(),
            google_token_url: GOOGLE_TOKEN_URL.into(),
            site_admin_email: var("NICGIT_SITE_ADMIN_EMAIL"),
        })
    }

    pub fn database_url_from_env() -> String {
        var("NICGIT_DATABASE_URL").unwrap_or_else(|| DEFAULT_DATABASE_URL.into())
    }

    pub fn for_tests(database_url: &str, data_dir: &Path, worker_token: &str) -> Config {
        Config {
            database_url: database_url.into(),
            data_dir: data_dir.into(),
            bind: "127.0.0.1:0".into(),
            worker_token: worker_token.into(),
            public_url: "http://localhost".into(),
            ai_router_url: None,
            ai_router_key: None,
            max_upload_mb: 64,
            web_dir: None,
            git: "git".into(),
            hook_exe: PathBuf::new(),
            google_client_id: None,
            google_client_secret: None,
            google_auth_url: GOOGLE_AUTH_URL.into(),
            google_token_url: GOOGLE_TOKEN_URL.into(),
            site_admin_email: None,
        }
    }

    pub fn https(&self) -> bool {
        self.public_url.starts_with("https://")
    }
}
