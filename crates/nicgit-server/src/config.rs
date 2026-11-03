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
}

fn var(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

impl Config {
    pub fn from_env() -> anyhow::Result<Config> {
        let bind = var("NICGIT_BIND").unwrap_or_else(|| "127.0.0.1:3000".into());
        Ok(Config {
            database_url: var("NICGIT_DATABASE_URL")
                .unwrap_or_else(|| "postgres://nicgit:nicgit@127.0.0.1:5434/nicgit".into()),
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
        })
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
        }
    }

    pub fn https(&self) -> bool {
        self.public_url.starts_with("https://")
    }
}
