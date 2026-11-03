// SPDX-License-Identifier: AGPL-3.0-or-later
use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};

/// A person's role on a project, ordered from least to most rights (spec section 8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Viewer,
    Reviewer,
    Member,
    Lead,
    Admin,
}

impl Role {
    pub const ALL: [Role; 5] = [Role::Viewer, Role::Reviewer, Role::Member, Role::Lead, Role::Admin];

    pub fn as_str(self) -> &'static str {
        match self {
            Role::Viewer => "viewer",
            Role::Reviewer => "reviewer",
            Role::Member => "member",
            Role::Lead => "lead",
            Role::Admin => "admin",
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct UnknownRole(pub String);

impl FromStr for Role {
    type Err = UnknownRole;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Role::ALL.into_iter().find(|r| r.as_str() == s).ok_or_else(|| UnknownRole(s.to_string()))
    }
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roles_are_ordered() {
        assert!(Role::Viewer < Role::Reviewer);
        assert!(Role::Reviewer < Role::Member);
        assert!(Role::Member < Role::Lead);
        assert!(Role::Lead < Role::Admin);
        assert_eq!("lead".parse::<Role>().unwrap(), Role::Lead);
        assert_eq!(Role::Admin.as_str(), "admin");
        assert!("owner".parse::<Role>().is_err());
    }
}
