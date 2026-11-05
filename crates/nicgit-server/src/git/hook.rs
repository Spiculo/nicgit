// SPDX-License-Identifier: AGPL-3.0-or-later
//! `nicgit-server hook pre-receive`: run by git inside a push, before any ref moves (spec section 11).
//! It gets the pusher's role and the protected options from the environment the server sets, and
//! rejects pushes that would break the project's rules or put invalid records in history.
use nicgit_core::roles::Role;
use std::io::{BufRead, Write};
use std::process::{Command, Stdio};

/// Paths whose blobs must be JSON objects: KimBim records and project files.
fn must_be_json(path: &str) -> bool {
    ["objects/", "nodes/", "loose/", "geometry/"].iter().any(|p| path.starts_with(p) && path.ends_with(".json"))
        || matches!(path, "kimbim.json" | "settings.json" | "checks.json")
}

fn is_zero(id: &str) -> bool {
    id.bytes().all(|b| b == b'0')
}

fn git(args: &[&str]) -> Result<String, String> {
    let git = std::env::var("NICGIT_GIT").unwrap_or_else(|_| "git".into());
    let out = Command::new(git).args(args).output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

pub struct Update {
    pub old: String,
    pub new: String,
    pub name: String,
}

/// Check one ref update against the rules; `Err` carries the message shown to the pusher.
pub fn check_ref(u: &Update, role: Role, protected: &[String]) -> Result<(), String> {
    let (option, is_tag) = if let Some(b) = u.name.strip_prefix("refs/heads/") {
        (Some(b), false)
    } else if u.name.starts_with("refs/tags/") {
        (None, true)
    } else {
        return Err(format!("{} is reserved; push design options (branches) and milestones (tags) only", u.name));
    };
    if role < Role::Member {
        return Err("your role on this project does not allow pushing".into());
    }
    if is_tag && !is_zero(&u.old) && role < Role::Lead {
        return Err(format!("milestone {} can only be moved or deleted by a lead", &u.name[10..]));
    }
    if let Some(opt) = option.filter(|o| protected.iter().any(|p| p == o)) {
        if role < Role::Lead {
            return Err(format!("design option '{opt}' is protected: combine a proposal into it instead"));
        }
        if is_zero(&u.new) {
            return Err(format!("design option '{opt}' is protected and cannot be deleted"));
        }
        if !is_zero(&u.old) && git(&["merge-base", "--is-ancestor", &u.old, &u.new]).is_err() {
            return Err(format!("design option '{opt}' is protected: its history cannot be rewritten"));
        }
    }
    Ok(())
}

/// A job's limit on what it may push, from `NICGIT_JOB_REFS`: `exact:<ref>`, or `create` for new refs only.
pub fn check_job_ref(u: &Update, rule: &str) -> Result<(), String> {
    if rule == "create" {
        if !is_zero(&u.old) || is_zero(&u.new) {
            return Err(format!("this import can only add new refs, not change {}", u.name));
        }
        return Ok(());
    }
    if rule.strip_prefix("exact:") == Some(u.name.as_str()) {
        return Ok(());
    }
    Err(format!("this job cannot change {}", u.name))
}

/// (path, blob id) of every blob that new commits added or changed.
fn new_blobs(u: &Update) -> Result<Vec<(String, String)>, String> {
    if is_zero(&u.new) {
        return Ok(vec![]);
    }
    let commits = git(&["rev-list", &u.new, "--not", "--all"])?;
    let mut out = Vec::new();
    for c in commits.lines().filter(|l| !l.is_empty()) {
        let diff = git(&["diff-tree", "-r", "--root", "-m", "--first-parent", "--no-commit-id", "--diff-filter=AMT", c])?;
        for line in diff.lines() {
            // ":old_mode new_mode old_id new_id status\tpath"
            let Some((meta, path)) = line.split_once('\t') else { continue };
            let fields: Vec<&str> = meta.split_whitespace().collect();
            if fields.len() >= 4 && must_be_json(path) {
                out.push((path.to_string(), fields[3].to_string()));
            }
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}

/// Read blobs with one `git cat-file --batch` and return the paths whose content is not a JSON object.
fn invalid_json(blobs: &[(String, String)]) -> Result<Vec<String>, String> {
    if blobs.is_empty() {
        return Ok(vec![]);
    }
    let git = std::env::var("NICGIT_GIT").unwrap_or_else(|_| "git".into());
    let mut child = Command::new(git)
        .args(["cat-file", "--batch"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    let mut stdin = child.stdin.take().expect("piped");
    let ids: Vec<String> = blobs.iter().map(|(_, id)| id.clone()).collect();
    let writer = std::thread::spawn(move || {
        for id in ids {
            if writeln!(stdin, "{id}").is_err() {
                break;
            }
        }
    });
    let mut reader = std::io::BufReader::new(child.stdout.take().expect("piped"));
    let mut bad = Vec::new();
    for (path, _) in blobs {
        let mut header = String::new();
        reader.read_line(&mut header).map_err(|e| e.to_string())?;
        let size: usize = header.split_whitespace().nth(2).and_then(|s| s.parse().ok()).ok_or("cat-file: bad header")?;
        let mut buf = vec![0u8; size + 1]; // content + trailing newline
        std::io::Read::read_exact(&mut reader, &mut buf).map_err(|e| e.to_string())?;
        buf.pop();
        let ok = matches!(serde_json::from_slice::<serde_json::Value>(&buf), Ok(serde_json::Value::Object(_)));
        if !ok {
            bad.push(path.clone());
        }
    }
    let _ = writer.join();
    let _ = child.wait();
    Ok(bad)
}

/// Entry point; returns the process exit code.
pub fn pre_receive() -> i32 {
    let role: Role = std::env::var("NICGIT_ROLE").ok().and_then(|r| r.parse().ok()).unwrap_or(Role::Viewer);
    let protected: Vec<String> = std::env::var("NICGIT_PROTECTED")
        .unwrap_or_default()
        .split('\u{1f}')
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    let job_rule = std::env::var("NICGIT_JOB_REFS").ok();
    let stdin = std::io::stdin();
    let mut updates = Vec::new();
    for line in stdin.lock().lines().map_while(Result::ok) {
        let mut parts = line.splitn(3, ' ');
        if let (Some(old), Some(new), Some(name)) = (parts.next(), parts.next(), parts.next()) {
            updates.push(Update { old: old.into(), new: new.into(), name: name.into() });
        }
    }
    let mut errors = Vec::new();
    for u in &updates {
        if let Some(Err(e)) = job_rule.as_deref().map(|rule| check_job_ref(u, rule)) {
            errors.push(e);
            continue;
        }
        if let Err(e) = check_ref(u, role, &protected) {
            errors.push(e);
            continue;
        }
        match new_blobs(u).and_then(|b| invalid_json(&b)) {
            Ok(bad) if bad.is_empty() => {}
            Ok(bad) => {
                let shown: Vec<&str> = bad.iter().take(5).map(String::as_str).collect();
                errors.push(format!(
                    "{} record(s) are not valid KimBim JSON (a text merge or a manual edit?): {}{}",
                    bad.len(),
                    shown.join(", "),
                    if bad.len() > 5 { ", ..." } else { "" }
                ));
            }
            Err(e) => errors.push(format!("could not check the push: {e}")),
        }
    }
    if errors.is_empty() {
        return 0;
    }
    for e in errors {
        eprintln!("NicGit: {e}");
    }
    1
}
