// SPDX-License-Identifier: AGPL-3.0-or-later
//! AI through the owner's cloud AI router (spec section 13). Filled in Task 10.
use crate::{access::Project, app::AppState, error::ApiResult, proposals::Proposal};
use serde_json::Value;

pub async fn proposal_summary_cached(_state: &AppState, _project: &Project, _p: &Proposal) -> ApiResult<Option<Value>> {
    Ok(None)
}
