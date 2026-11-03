// SPDX-License-Identifier: AGPL-3.0-or-later
//! Git hosting: the HTTP transport, the pre-receive hook and small helpers over the git CLI.
pub mod hook;
pub mod http;

use serde::Serialize;
use std::collections::BTreeMap;
use std::path::Path;

/// ref name -> commit id
pub type RefMap = BTreeMap<String, String>;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RefUpdate {
    pub name: String,
    pub old: Option<String>,
    pub new: Option<String>,
}

impl RefUpdate {
    /// The design option name for `refs/heads/<name>`.
    pub fn option(&self) -> Option<&str> {
        self.name.strip_prefix("refs/heads/")
    }
}

/// All refs of a bare repository.
pub async fn refs(git: &str, git_dir: &Path) -> anyhow::Result<RefMap> {
    let out = tokio::process::Command::new(git)
        .arg("--git-dir")
        .arg(git_dir)
        .args(["for-each-ref", "--format=%(objectname) %(refname)"])
        .output()
        .await?;
    anyhow::ensure!(out.status.success(), "for-each-ref: {}", String::from_utf8_lossy(&out.stderr));
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| l.split_once(' '))
        .map(|(id, name)| (name.to_string(), id.to_string()))
        .collect())
}

pub fn changed(before: &RefMap, after: &RefMap) -> Vec<RefUpdate> {
    let mut out = Vec::new();
    for (name, new) in after {
        if before.get(name) != Some(new) {
            out.push(RefUpdate { name: name.clone(), old: before.get(name).cloned(), new: Some(new.clone()) });
        }
    }
    for (name, old) in before {
        if !after.contains_key(name) {
            out.push(RefUpdate { name: name.clone(), old: Some(old.clone()), new: None });
        }
    }
    out
}

/// Run git in a bare repository and return stdout.
pub async fn run(git: &str, git_dir: &Path, args: &[&str]) -> anyhow::Result<String> {
    let out = tokio::process::Command::new(git).arg("--git-dir").arg(git_dir).args(args).output().await?;
    anyhow::ensure!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr).trim());
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}
