// SPDX-License-Identifier: AGPL-3.0-or-later
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;

/// Which side wins a conflict. `Ours` is the target option, `Theirs` the source option (engine words).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Ours,
    Theirs,
}

/// How a proposal's conflicts are settled (decision N6). The browser never edits values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictMode {
    /// Only a merge without conflicts may combine.
    Auto,
    /// Target option wins every conflict.
    WholeOurs,
    /// Source option wins every conflict.
    WholeTheirs,
    /// A side is picked for each conflict.
    Granular,
}

impl ConflictMode {
    pub const ALL: [ConflictMode; 4] =
        [ConflictMode::Auto, ConflictMode::WholeOurs, ConflictMode::WholeTheirs, ConflictMode::Granular];

    pub fn as_str(self) -> &'static str {
        match self {
            ConflictMode::Auto => "auto",
            ConflictMode::WholeOurs => "whole:ours",
            ConflictMode::WholeTheirs => "whole:theirs",
            ConflictMode::Granular => "granular",
        }
    }
}

impl FromStr for ConflictMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        ConflictMode::ALL.into_iter().find(|m| m.as_str() == s).ok_or_else(|| format!("unknown conflict mode {s:?}"))
    }
}

/// Conflicts that still need a side.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[error("{} conflict(s) still need a side", .0.len())]
pub struct MissingPicks(pub Vec<String>);

/// The engine's `resolutions` (conflict id -> side) for a mode. Picks for unknown ids are ignored.
pub fn resolve(
    mode: ConflictMode,
    conflict_ids: &[String],
    picks: &HashMap<String, Side>,
) -> Result<HashMap<String, Side>, MissingPicks> {
    let all = |side| conflict_ids.iter().map(|id| (id.clone(), side)).collect();
    match mode {
        ConflictMode::Auto if conflict_ids.is_empty() => Ok(HashMap::new()),
        ConflictMode::Auto => Err(MissingPicks(conflict_ids.to_vec())),
        ConflictMode::WholeOurs => Ok(all(Side::Ours)),
        ConflictMode::WholeTheirs => Ok(all(Side::Theirs)),
        ConflictMode::Granular => {
            let missing: Vec<String> = conflict_ids.iter().filter(|id| !picks.contains_key(*id)).cloned().collect();
            if !missing.is_empty() {
                return Err(MissingPicks(missing));
            }
            Ok(conflict_ids.iter().map(|id| (id.clone(), picks[id])).collect())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn ids() -> Vec<String> {
        vec!["c1".into(), "c2".into()]
    }

    #[test]
    fn whole_theirs_picks_all() {
        let r = resolve(ConflictMode::WholeTheirs, &ids(), &HashMap::new()).unwrap();
        assert_eq!(r.len(), 2);
        assert!(r.values().all(|s| *s == Side::Theirs));
    }

    #[test]
    fn whole_ours_picks_all() {
        let r = resolve(ConflictMode::WholeOurs, &ids(), &HashMap::new()).unwrap();
        assert!(r.values().all(|s| *s == Side::Ours));
    }

    #[test]
    fn granular_requires_every_pick() {
        let picks = HashMap::from([("c1".to_string(), Side::Theirs)]);
        let err = resolve(ConflictMode::Granular, &ids(), &picks).unwrap_err();
        assert_eq!(err, MissingPicks(vec!["c2".into()]));
        let picks = HashMap::from([("c1".to_string(), Side::Theirs), ("c2".to_string(), Side::Ours)]);
        let r = resolve(ConflictMode::Granular, &ids(), &picks).unwrap();
        assert_eq!(r["c2"], Side::Ours);
    }

    #[test]
    fn auto_with_conflicts_fails() {
        assert!(resolve(ConflictMode::Auto, &ids(), &HashMap::new()).is_err());
        assert!(resolve(ConflictMode::Auto, &[], &HashMap::new()).unwrap().is_empty());
    }

    #[test]
    fn modes_parse() {
        assert_eq!("whole:theirs".parse::<ConflictMode>().unwrap(), ConflictMode::WholeTheirs);
        assert_eq!(ConflictMode::Granular.as_str(), "granular");
        assert!("merge".parse::<ConflictMode>().is_err());
    }
}
