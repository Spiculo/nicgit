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

/// A starting point for a user name, from a person's name: `Ana García` becomes `ana-garcia`. It may still be
/// reserved or taken: callers try `base`, `base-2`, `base-3`... None when nothing of the name can be used.
pub fn slug_base(name: &str) -> Option<String> {
    let folded = name.to_lowercase().chars().map(fold).collect::<String>();
    let mut mapped = String::with_capacity(folded.len());
    for c in folded.chars() {
        let c = if c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '_' | '.') { c } else { '-' };
        if c != '-' || !mapped.ends_with('-') {
            mapped.push(c);
        }
    }
    let trimmed = mapped.trim_start_matches(['-', '_', '.']);
    let short: String = trimmed.chars().take(56).collect();
    let mut out = short.trim_end_matches(['-', '_', '.']).to_string();
    while let Some(rest) = out.strip_suffix(".git") {
        out = rest.trim_end_matches(['-', '_', '.']).to_string();
    }
    (!out.is_empty()).then_some(out)
}

/// The accented letters of the languages NicGit is used in, as their plain letter.
fn fold(c: char) -> char {
    match c {
        'á' | 'à' | 'ä' | 'â' | 'ã' | 'å' => 'a',
        'é' | 'è' | 'ë' | 'ê' => 'e',
        'í' | 'ì' | 'ï' | 'î' => 'i',
        'ó' | 'ò' | 'ö' | 'ô' | 'õ' => 'o',
        'ú' | 'ù' | 'ü' | 'û' => 'u',
        'ñ' => 'n',
        'ç' => 'c',
        _ => c,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_base_rules() {
        assert_eq!(slug_base("Ana Ferreira").unwrap(), "ana-ferreira");
        assert_eq!(slug_base("José Núñez Ibáñez").unwrap(), "jose-nunez-ibanez");
        assert_eq!(slug_base("  Ana   + Bo ").unwrap(), "ana-bo");
        assert_eq!(slug_base("+++"), None);
        assert_eq!(slug_base(""), None);
        assert_eq!(slug_base("山田太郎"), None);
        assert_eq!(slug_base("_x_").unwrap(), "x");
        assert_eq!(slug_base("repo.git").unwrap(), "repo");
        assert_eq!(slug_base(&"a".repeat(100)).unwrap().len(), 56);
        assert!(valid_slug(&format!("{}-999", slug_base(&"a".repeat(100)).unwrap())));
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
