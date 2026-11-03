// SPDX-License-Identifier: AGPL-3.0-or-later
//! Whether a proposal has the approvals its target option's rule asks for (spec section 9).
use crate::roles::Role;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    Approve,
    Changes,
    Comment,
}

impl Verdict {
    pub fn parse(s: &str) -> Option<Verdict> {
        match s {
            "approve" => Some(Verdict::Approve),
            "changes" => Some(Verdict::Changes),
            "comment" => Some(Verdict::Comment),
            _ => None,
        }
    }
}

/// A reviewer's latest, still-valid verdict, with their role on the project.
#[derive(Debug, Clone)]
pub struct Review {
    pub user: String,
    pub role: Role,
    pub verdict: Verdict,
}

/// What the target option's rule asks for.
#[derive(Debug, Clone, Default)]
pub struct Requirement {
    pub approvals: u32,
    /// Each listed role needs an approval from someone with that role or higher.
    pub roles: Vec<Role>,
    /// Protected project settings the proposal changes: they need a lead's approval.
    pub protected_settings_changed: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Missing {
    Approvals { needed: u32, have: u32 },
    Role { role: Role },
    ChangesRequested { users: Vec<String> },
    SettingsApproval { keys: Vec<String> },
}

#[derive(Debug, Clone, Serialize)]
pub struct Evaluation {
    pub ok: bool,
    pub approvals: u32,
    pub missing: Vec<Missing>,
}

/// One review per user is expected (the latest); approvals by distinct users count once each.
pub fn evaluate(req: &Requirement, reviews: &[Review]) -> Evaluation {
    let approvers: Vec<&Review> = reviews.iter().filter(|r| r.verdict == Verdict::Approve).collect();
    let mut missing = Vec::new();
    let blockers: Vec<String> = reviews.iter().filter(|r| r.verdict == Verdict::Changes).map(|r| r.user.clone()).collect();
    if !blockers.is_empty() {
        missing.push(Missing::ChangesRequested { users: blockers });
    }
    let have = approvers.len() as u32;
    if have < req.approvals {
        missing.push(Missing::Approvals { needed: req.approvals, have });
    }
    for role in &req.roles {
        if !approvers.iter().any(|r| r.role >= *role) {
            missing.push(Missing::Role { role: *role });
        }
    }
    if !req.protected_settings_changed.is_empty() && !approvers.iter().any(|r| r.role >= Role::Lead) {
        missing.push(Missing::SettingsApproval { keys: req.protected_settings_changed.clone() });
    }
    Evaluation { ok: missing.is_empty(), approvals: have, missing }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::roles::Role;

    fn review(user: &str, role: Role, verdict: Verdict) -> Review {
        Review { user: user.into(), role, verdict }
    }

    #[test]
    fn unprotected_option_needs_nothing() {
        let r = evaluate(&Requirement::default(), &[]);
        assert!(r.ok, "{r:?}");
    }

    #[test]
    fn counts_distinct_approvers() {
        let req = Requirement { approvals: 2, ..Default::default() };
        let one = [review("ana", Role::Reviewer, Verdict::Approve)];
        let r = evaluate(&req, &one);
        assert!(!r.ok);
        assert_eq!(r.missing, vec![Missing::Approvals { needed: 2, have: 1 }]);
        let two = [review("ana", Role::Reviewer, Verdict::Approve), review("bo", Role::Member, Verdict::Approve)];
        assert!(evaluate(&req, &two).ok);
    }

    #[test]
    fn required_role_needs_that_role_or_higher() {
        let req = Requirement { approvals: 1, roles: vec![Role::Lead], ..Default::default() };
        let r = evaluate(&req, &[review("bo", Role::Member, Verdict::Approve)]);
        assert_eq!(r.missing, vec![Missing::Role { role: Role::Lead }]);
        assert!(evaluate(&req, &[review("al", Role::Admin, Verdict::Approve)]).ok);
    }

    #[test]
    fn requested_changes_block() {
        let req = Requirement { approvals: 1, ..Default::default() };
        let r = evaluate(&req, &[review("ana", Role::Lead, Verdict::Approve), review("bo", Role::Reviewer, Verdict::Changes)]);
        assert_eq!(r.missing, vec![Missing::ChangesRequested { users: vec!["bo".into()] }]);
    }

    #[test]
    fn protected_settings_need_a_lead() {
        let req = Requirement { approvals: 1, protected_settings_changed: vec!["georeference".into()], ..Default::default() };
        let r = evaluate(&req, &[review("bo", Role::Member, Verdict::Approve)]);
        assert_eq!(r.missing, vec![Missing::SettingsApproval { keys: vec!["georeference".into()] }]);
        assert!(evaluate(&req, &[review("al", Role::Lead, Verdict::Approve)]).ok);
    }

    #[test]
    fn every_missing_kind_serializes() {
        let all = vec![
            Missing::Approvals { needed: 2, have: 0 },
            Missing::Role { role: Role::Lead },
            Missing::ChangesRequested { users: vec!["bo".into()] },
            Missing::SettingsApproval { keys: vec!["units".into()] },
        ];
        let v = serde_json::to_value(&all).expect("serializes");
        assert_eq!(v[1]["kind"], "role");
        assert_eq!(v[1]["role"], "lead");
        assert_eq!(v[2]["users"][0], "bo");
        assert_eq!(v[3]["keys"][0], "units");
    }

    #[test]
    fn comments_do_not_count() {
        let req = Requirement { approvals: 1, ..Default::default() };
        assert!(!evaluate(&req, &[review("ana", Role::Lead, Verdict::Comment)]).ok);
    }
}
