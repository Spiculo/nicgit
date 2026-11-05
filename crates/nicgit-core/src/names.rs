// SPDX-License-Identifier: AGPL-3.0-or-later
/// Words that are routes of the web app or the API, so no account or project may take them.
pub const RESERVED: &[&str] = &[
    "api", "login", "logout", "signup", "new", "settings", "explore", "assets", "static", "internal", "git",
    "import", "orgs", "admin",
];

/// An account or project slug: lowercase ASCII letters, digits, `-`, `_`, `.`, starting with a letter or
/// digit, at most 63 characters, not ending in `.git` and not reserved.
pub fn valid_slug(s: &str) -> bool {
    let b = s.as_bytes();
    if b.is_empty() || b.len() > 63 || !(b[0].is_ascii_lowercase() || b[0].is_ascii_digit()) {
        return false;
    }
    if !b.iter().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'-' | b'_' | b'.')) {
        return false;
    }
    !s.ends_with(".git") && !RESERVED.contains(&s)
}

/// A starting point for a user name, from the part of an email address before the `@`. It may still be
/// reserved or taken: callers try `base`, `base-2`, `base-3`...
pub fn slug_base(email: &str) -> String {
    let local = email.split('@').next().unwrap_or("").to_lowercase();
    let mapped: String = local.chars().map(|c| if c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '-' | '_' | '.') { c } else { '-' }).collect();
    let trimmed = mapped.trim_start_matches(['-', '_', '.']);
    let short: String = trimmed.chars().take(56).collect();
    let mut out = short.trim_end_matches(['-', '_', '.']).to_string();
    while let Some(rest) = out.strip_suffix(".git") {
        out = rest.trim_end_matches(['-', '_', '.']).to_string();
    }
    if out.is_empty() { "user".to_string() } else { out }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_base_rules() {
        assert_eq!(slug_base("Ana.Ferreira@example.com"), "ana.ferreira");
        assert_eq!(slug_base("a+b@example.com"), "a-b");
        assert_eq!(slug_base("+++@example.com"), "user");
        assert_eq!(slug_base("@example.com"), "user");
        assert_eq!(slug_base("_x_@example.com"), "x");
        assert_eq!(slug_base("repo.git@example.com"), "repo");
        assert_eq!(slug_base(&format!("{}@example.com", "a".repeat(100))).len(), 56);
        assert!(valid_slug(&format!("{}-999", slug_base(&format!("{}@e.c", "a".repeat(100))))));
    }

    #[test]
    fn slug_rules() {
        assert!(valid_slug("house-1"));
        assert!(valid_slug("a"));
        assert!(valid_slug("clinic_v2.final"));
        assert!(!valid_slug("api"));
        assert!(!valid_slug("-x"));
        assert!(!valid_slug("a.git"));
        assert!(!valid_slug("House"));
        assert!(!valid_slug(""));
        assert!(!valid_slug(&"a".repeat(64)));
        assert!(valid_slug(&"a".repeat(63)));
    }
}
