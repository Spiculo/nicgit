// SPDX-License-Identifier: AGPL-3.0-or-later
//! Helpers that build small NicBim-shaped repositories with the plain git CLI.
use std::path::Path;
use std::process::Output;

pub async fn git(dir: &Path, args: &[&str]) -> Output {
    tokio::process::Command::new("git")
        .current_dir(dir)
        .args(["-c", "credential.helper=", "-c", "user.name=Test", "-c", "user.email=test@example.com", "-c", "init.defaultBranch=main"])
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "never")
        .output()
        .await
        .expect("git runs")
}

/// Run git and return trimmed stdout; panics with stderr on failure.
pub async fn git_ok(dir: &Path, args: &[&str]) -> String {
    let out = git(dir, args).await;
    assert!(out.status.success(), "git {args:?} failed: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

pub fn write_record(dir: &Path, path: &str, content: &str) {
    let p = dir.join(path);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, content).unwrap();
}

/// A working repository on `main` with one version of a tiny NicBim project.
pub async fn kimbim_repo() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    git_ok(d.path(), &["init", "-q", "-b", "main"]).await;
    write_record(d.path(), "kimbim.json", "{\"k\":\"kimbim/1\",\"schema\":\"IFC4\"}");
    write_record(d.path(), "settings.json", "{\"units\":\"mm\"}");
    write_record(d.path(), "objects/ab/0Zukam4n1BpBYDaJ2UpTma.json", "{\"a\":{\"GlobalId\":\"0Zukam4n1BpBYDaJ2UpTma\",\"Name\":\"W-1\"},\"t\":\"IfcWall\"}");
    write_record(d.path(), "nodes/0f/0f1aa794d8198ccee0ecb544a898b6e8.json", "{\"a\":{\"Name\":\"Pset_WallCommon\"},\"t\":\"IfcPropertySet\"}");
    git_ok(d.path(), &["add", "-A"]).await;
    git_ok(d.path(), &["commit", "-qm", "The first version"]).await;
    d
}
