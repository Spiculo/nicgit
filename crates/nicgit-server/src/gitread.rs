// SPDX-License-Identifier: AGPL-3.0-or-later
//! Reading a project's history through the NicBim engine's own repository crate (`kimbim-repo`, gix), so
//! the server sees exactly what the desktop app sees. Calls are blocking; they run on the blocking pool.
use crate::error::{ApiResult, AppError};
use kimbim_repo::{CommitInfo, Repo};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

/// A revision the user may name: a design option, a tag or a commit, with `~`/`^` suffixes. Anything that
/// could be read as a git flag or a path is refused.
pub fn valid_rev(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 255
        && !s.starts_with('-')
        && !s.contains("..")
        && !s.contains("@{")
        && !s.chars().any(|c| c.is_control() || matches!(c, ' ' | ':' | '\\' | '?' | '*' | '['))
}

/// A path inside a version's tree.
pub fn valid_path(s: &str) -> bool {
    !s.starts_with('/') && !s.contains('\\') && !s.split('/').any(|p| p == ".." || p == "." ) && !s.contains(':') && !s.chars().any(char::is_control)
}

#[derive(Debug, Clone, Serialize)]
pub struct Version {
    pub id: String,
    pub parents: Vec<String>,
    pub author: String,
    pub email: String,
    pub time: i64,
    pub message: String,
}

impl From<CommitInfo> for Version {
    fn from(c: CommitInfo) -> Self {
        Version { id: c.id, parents: c.parents, author: c.author_name, email: c.author_email, time: c.time, message: c.message.trim_end().to_string() }
    }
}

pub struct Git {
    pub project_dir: PathBuf,
}

fn err(e: kimbim_repo::Error) -> AppError {
    match e {
        kimbim_repo::Error::UnknownRev(_) | kimbim_repo::Error::UnknownBranch(_) => AppError::not_found(),
        other => AppError::internal(other),
    }
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> ApiResult<T> + Send + 'static) -> ApiResult<T> {
    tokio::task::spawn_blocking(f).await.map_err(AppError::internal)?
}

impl Git {
    pub fn new(project_dir: &Path) -> Git {
        Git { project_dir: project_dir.to_path_buf() }
    }

    fn open(dir: &Path) -> ApiResult<Repo> {
        Repo::open(dir).map_err(err)
    }

    pub async fn options(&self) -> ApiResult<Vec<String>> {
        let dir = self.project_dir.clone();
        blocking(move || Self::open(&dir)?.branches().map_err(err)).await
    }

    /// Resolve a user-given revision to a commit id: a design option first, then any revision.
    pub async fn resolve(&self, rev: &str) -> ApiResult<String> {
        if !valid_rev(rev) {
            return Err(AppError::bad_request("not a version, option or milestone name"));
        }
        let dir = self.project_dir.clone();
        let rev = rev.to_string();
        blocking(move || {
            let repo = Self::open(&dir)?;
            repo.resolve(&format!("refs/heads/{rev}")).or_else(|_| repo.resolve(&rev)).map_err(err)
        })
        .await
    }

    pub async fn log(&self, commit: &str, limit: usize) -> ApiResult<Vec<Version>> {
        let dir = self.project_dir.clone();
        let commit = commit.to_string();
        blocking(move || Ok(Self::open(&dir)?.log(&commit, limit).map_err(err)?.into_iter().map(Version::from).collect())).await
    }

    pub async fn tags(&self) -> ApiResult<Vec<Value>> {
        let dir = self.project_dir.clone();
        blocking(move || {
            Ok(Self::open(&dir)?
                .tags()
                .map_err(err)?
                .into_iter()
                .map(|t| json!({"name": t.name, "target": t.target, "message": t.message.trim_end(), "tagger": t.tagger_name, "time": t.time}))
                .collect())
        })
        .await
    }

    pub async fn blob(&self, commit: &str, path: &str) -> ApiResult<Option<Vec<u8>>> {
        let dir = self.project_dir.clone();
        let (commit, path) = (commit.to_string(), path.to_string());
        blocking(move || Self::open(&dir)?.read_blob_at(&commit, &path).map_err(err)).await
    }

    /// Record counts of a version, cached by commit (commits never change).
    pub async fn counts(&self, commit: &str) -> ApiResult<Value> {
        static CACHE: LazyLock<Mutex<HashMap<String, Value>>> = LazyLock::new(Default::default);
        if let Some(v) = CACHE.lock().unwrap().get(commit) {
            return Ok(v.clone());
        }
        let dir = self.project_dir.clone();
        let c = commit.to_string();
        let counts = blocking(move || {
            let entries = Self::open(&dir)?.tree_entries(&c).map_err(err)?;
            let mut n: HashMap<&str, u64> = HashMap::new();
            for path in entries.keys() {
                let top = path.split('/').next().unwrap_or("");
                let kind = match top {
                    "objects" | "nodes" | "loose" | "geometry" | "files" => top,
                    _ => "other",
                };
                *n.entry(kind).or_default() += 1;
            }
            Ok(json!(n))
        })
        .await?;
        let mut cache = CACHE.lock().unwrap();
        if cache.len() > 10_000 {
            cache.clear();
        }
        cache.insert(commit.to_string(), counts.clone());
        Ok(counts)
    }

    /// Changed record paths between two commits (the engine's tree diff, which skips identical subtrees).
    pub async fn diff_paths(&self, a: &str, b: &str) -> ApiResult<Vec<(String, Option<String>, Option<String>)>> {
        let dir = self.project_dir.clone();
        let (a, b) = (a.to_string(), b.to_string());
        blocking(move || Self::open(&dir)?.diff_entries(&a, &b).map_err(err)).await
    }

    pub async fn merge_base(&self, a: &str, b: &str) -> ApiResult<Option<String>> {
        let dir = self.project_dir.clone();
        let (a, b) = (a.to_string(), b.to_string());
        blocking(move || Self::open(&dir)?.merge_base(&a, &b).map_err(err)).await
    }
}
