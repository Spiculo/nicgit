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

#[cfg(test)]
mod tests {
    use super::*;

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
