// SPDX-License-Identifier: AGPL-3.0-or-later
//! Project repositories on disk: `<data>/repos/<project id>/.git`, a bare Git directory named `.git` so the
//! KimBim engine opens the folder as a project unchanged (ledger ruling, Task 1).
use crate::config::Config;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub fn dir(cfg: &Config, project: Uuid) -> PathBuf {
    cfg.data_dir.join("repos").join(project.to_string())
}

pub fn git_dir(cfg: &Config, project: Uuid) -> PathBuf {
    dir(cfg, project).join(".git")
}

/// Everything cached for one project.
pub fn cache_dir(cfg: &Config, project: Uuid) -> PathBuf {
    cfg.data_dir.join("cache").join(project.to_string())
}

/// Uploaded files waiting for their import job, per project.
pub fn uploads_dir(cfg: &Config, project: Uuid) -> PathBuf {
    cfg.data_dir.join("uploads").join(project.to_string())
}

/// Files still arriving, before the project they will belong to exists.
pub fn incoming_dir(cfg: &Config) -> PathBuf {
    cfg.data_dir.join("uploads").join("incoming")
}

/// One plain file name: no separators, not hidden, nothing a filesystem could read as a path.
fn plain_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 200
        && !name.starts_with('.')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '%'))
}

pub fn upload_path(cfg: &Config, project: Uuid, name: &str) -> Option<PathBuf> {
    plain_name(name).then(|| uploads_dir(cfg, project).join(name))
}

/// `<folder>/<name>` inside the project's cache. Job results name artifacts this way, and they are not trusted.
pub fn artifact_path(cfg: &Config, project: Uuid, path: &str) -> Option<PathBuf> {
    let (folder, name) = path.split_once('/')?;
    (matches!(folder, "ifc" | "exports") && plain_name(name)).then(|| cache_dir(cfg, project).join(folder).join(name))
}

/// Point the repository's HEAD at a design option.
pub async fn set_head(cfg: &Config, project: Uuid, option: &str) -> anyhow::Result<()> {
    crate::git::run(&cfg.git, &git_dir(cfg, project), &["symbolic-ref", "HEAD", &format!("refs/heads/{option}")]).await?;
    Ok(())
}

/// The pre-receive hook: git runs it with `sh`, on Windows too (Git for Windows).
pub fn hook_script(hook_exe: &Path) -> String {
    let exe = hook_exe.to_string_lossy().replace('\\', "/");
    format!("#!/bin/sh\n# Installed by NicGit. Validates pushes (spec section 11).\nexec \"{exe}\" hook pre-receive\n")
}

pub async fn init(cfg: &Config, project: Uuid) -> anyhow::Result<PathBuf> {
    let git = git_dir(cfg, project);
    std::fs::create_dir_all(&git)?;
    let out = tokio::process::Command::new(&cfg.git).arg("init").arg("--bare").arg("-q").arg(&git).output().await?;
    anyhow::ensure!(out.status.success(), "git init failed: {}", String::from_utf8_lossy(&out.stderr));
    std::fs::write(git.join("HEAD"), "ref: refs/heads/main\n")?;
    for (k, v) in [
        ("core.logAllRefUpdates", "false"),
        ("receive.fsckObjects", "false"),
        ("uploadpack.allowFilter", "true"),
        ("uploadpack.allowAnySHA1InWant", "false"),
        ("gc.auto", "0"),
    ] {
        let out = tokio::process::Command::new(&cfg.git).arg("--git-dir").arg(&git).args(["config", k, v]).output().await?;
        anyhow::ensure!(out.status.success(), "git config {k} failed");
    }
    install_hooks(cfg, &git)?;
    Ok(git)
}

pub fn install_hooks(cfg: &Config, git: &Path) -> anyhow::Result<()> {
    let hooks = git.join("hooks");
    std::fs::create_dir_all(&hooks)?;
    for entry in std::fs::read_dir(&hooks)? {
        let p = entry?.path();
        if p.extension().is_some_and(|e| e == "sample") {
            std::fs::remove_file(p)?;
        }
    }
    let path = hooks.join("pre-receive");
    std::fs::write(&path, hook_script(&cfg.hook_exe))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}

/// Remove a folder, including the read-only pack files Git writes on Windows.
pub fn remove_all(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        return Ok(());
    }
    fn make_writable(p: &Path) -> std::io::Result<()> {
        for entry in std::fs::read_dir(p)? {
            let e = entry?;
            let ft = e.file_type()?;
            if ft.is_dir() {
                make_writable(&e.path())?;
            } else {
                let mut perm = e.metadata()?.permissions();
                #[allow(clippy::permissions_set_readonly_false)]
                perm.set_readonly(false);
                std::fs::set_permissions(e.path(), perm)?;
            }
        }
        Ok(())
    }
    make_writable(path)?;
    std::fs::remove_dir_all(path)
}
