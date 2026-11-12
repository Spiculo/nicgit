// SPDX-License-Identifier: AGPL-3.0-or-later
use std::path::{Path, PathBuf};

/// Which of the two ways NicGit is run (docs/deployment.md, decision N38). One binary and one set of images
/// serve both; this switch only changes defaults that differ between them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Deployment {
    /// Our service: anyone may join, storage is capped per account, usage is metered for billing.
    Hosted,
    /// A studio's own server, on premises or on its own cloud: only the studio's people join.
    SelfHosted,
}

impl Deployment {
    pub fn parse(s: &str) -> Option<Deployment> {
        match s.trim() {
            "hosted" => Some(Deployment::Hosted),
            "self-hosted" => Some(Deployment::SelfHosted),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Deployment::Hosted => "hosted",
            Deployment::SelfHosted => "self-hosted",
        }
    }
}

/// Who may create an account by signing in (decision N39). Existing accounts can always sign in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signup {
    /// Anyone with a verified Google account.
    pub open: bool,
    /// When not open, verified addresses at these domains (lowercase, no `@`) may still join.
    pub domains: Vec<String>,
}

impl Signup {
    /// `NICGIT_SIGNUP` (`open` or `closed`, default from the deployment) and `NICGIT_SIGNUP_DOMAINS` (comma list).
    pub fn from_settings(deployment: Deployment, signup: Option<&str>, domains: Option<&str>) -> anyhow::Result<Signup> {
        let open = match signup.map(str::trim) {
            None => deployment == Deployment::Hosted,
            Some("open") => true,
            Some("closed") => false,
            Some(other) => anyhow::bail!("NICGIT_SIGNUP must be `open` or `closed`, not `{other}`"),
        };
        let domains = domains
            .unwrap_or("")
            .split(',')
            .map(|d| d.trim().trim_start_matches('@').to_lowercase())
            .filter(|d| !d.is_empty())
            .collect::<Vec<_>>();
        if let Some(bad) = domains.iter().find(|d| !d.contains('.') || d.contains('@') || d.contains(char::is_whitespace)) {
            anyhow::bail!("NICGIT_SIGNUP_DOMAINS: `{bad}` is not a domain");
        }
        Ok(Signup { open, domains })
    }

    /// Whether a new account may be made for this verified address. The site admin always may, so a closed
    /// server can be set up by the person who runs it.
    pub fn allows(&self, email: &str, site_admin: bool) -> bool {
        site_admin
            || self.open
            || email.rsplit_once('@').is_some_and(|(_, domain)| self.domains.iter().any(|d| d.eq_ignore_ascii_case(domain)))
    }
}

/// Server configuration, read from `NICGIT_*` environment variables.
#[derive(Debug, Clone)]
pub struct Config {
    pub deployment: Deployment,
    pub signup: Signup,
    pub database_url: String,
    pub data_dir: PathBuf,
    pub bind: String,
    pub worker_token: String,
    pub public_url: String,
    pub ai_router_url: Option<String>,
    pub ai_router_key: Option<String>,
    /// Where engine workers connect. Nothing else needs this address, so it is never published (decision N35).
    pub internal_bind: String,
    /// The largest single file: an upload, or a file in a push (decision N33).
    pub max_file_mb: u64,
    /// The most one project's repository may hold, and separately its cached exports.
    pub max_repo_mb: u64,
    /// The most an account may hold across its projects.
    pub max_account_mb: u64,
    pub max_projects: u64,
    pub max_orgs: u64,
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

fn number(name: &str, default: u64) -> u64 {
    var(name).and_then(|v| v.parse().ok()).unwrap_or(default)
}

impl Config {
    pub fn from_env() -> anyhow::Result<Config> {
        let bind = var("NICGIT_BIND").unwrap_or_else(|| "127.0.0.1:3000".into());
        // No default: the two deployments differ in who may join, so the operator says which this is.
        let deployment = var("NICGIT_DEPLOYMENT")
            .as_deref()
            .and_then(Deployment::parse)
            .ok_or_else(|| anyhow::anyhow!("NICGIT_DEPLOYMENT must be `hosted` or `self-hosted` (docs/deployment.md)"))?;
        let signup = Signup::from_settings(deployment, var("NICGIT_SIGNUP").as_deref(), var("NICGIT_SIGNUP_DOMAINS").as_deref())?;
        Ok(Config {
            deployment,
            signup,
            database_url: Self::database_url_from_env(),
            data_dir: PathBuf::from(var("NICGIT_DATA_DIR").unwrap_or_else(|| "data".into())),
            public_url: var("NICGIT_PUBLIC_URL").unwrap_or_else(|| format!("http://{bind}")),
            bind,
            worker_token: var("NICGIT_WORKER_TOKEN")
                .ok_or_else(|| anyhow::anyhow!("NICGIT_WORKER_TOKEN must be set (shared with engine workers)"))?,
            ai_router_url: var("NICGIT_AI_ROUTER_URL"),
            ai_router_key: var("NICGIT_AI_ROUTER_KEY"),
            internal_bind: var("NICGIT_INTERNAL_BIND").unwrap_or_else(|| "127.0.0.1:3001".into()),
            max_file_mb: number("NICGIT_MAX_FILE_MB", 512),
            max_repo_mb: number("NICGIT_MAX_REPO_MB", 2048),
            max_account_mb: number("NICGIT_MAX_ACCOUNT_MB", 5120),
            max_projects: number("NICGIT_MAX_PROJECTS", 100),
            max_orgs: number("NICGIT_MAX_ORGS", 3),
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
            deployment: Deployment::Hosted,
            signup: Signup { open: true, domains: vec![] },
            database_url: database_url.into(),
            data_dir: data_dir.into(),
            bind: "127.0.0.1:0".into(),
            worker_token: worker_token.into(),
            public_url: "http://localhost".into(),
            ai_router_url: None,
            ai_router_key: None,
            internal_bind: "127.0.0.1:0".into(),
            max_file_mb: 64,
            max_repo_mb: 1024,
            max_account_mb: 4096,
            max_projects: 100,
            max_orgs: 100,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deployment_names_round_trip() {
        for d in [Deployment::Hosted, Deployment::SelfHosted] {
            assert_eq!(Deployment::parse(d.name()), Some(d));
        }
        assert_eq!(Deployment::parse("onprem"), None);
        assert_eq!(Deployment::parse(""), None);
    }

    #[test]
    fn signup_defaults_follow_the_deployment() {
        assert!(Signup::from_settings(Deployment::Hosted, None, None).unwrap().open);
        assert!(!Signup::from_settings(Deployment::SelfHosted, None, None).unwrap().open);
        assert!(Signup::from_settings(Deployment::SelfHosted, Some("open"), None).unwrap().open);
        assert!(!Signup::from_settings(Deployment::Hosted, Some("closed"), None).unwrap().open);
        assert!(Signup::from_settings(Deployment::Hosted, Some("sometimes"), None).is_err());
    }

    #[test]
    fn signup_domains_are_normalised_and_checked() {
        let s = Signup::from_settings(Deployment::SelfHosted, None, Some(" Studio.example , @branch.example.org,,")).unwrap();
        assert_eq!(s.domains, ["studio.example", "branch.example.org"]);
        assert!(Signup::from_settings(Deployment::SelfHosted, None, Some("localhost")).is_err());
        assert!(Signup::from_settings(Deployment::SelfHosted, None, Some("ana@studio.example")).is_err());
    }

    #[test]
    fn closed_signup_admits_listed_domains_and_the_admin_only() {
        let s = Signup { open: false, domains: vec!["studio.example".into()] };
        assert!(s.allows("ana@studio.example", false));
        assert!(s.allows("ana@STUDIO.example", false));
        assert!(!s.allows("ana@sub.studio.example", false));
        assert!(!s.allows("ana@studio.example.evil", false));
        assert!(!s.allows("ana@gmail.com", false));
        assert!(s.allows("boss@gmail.com", true));
        assert!(Signup { open: true, domains: vec![] }.allows("anyone@gmail.com", false));
    }
}
