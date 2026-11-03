// SPDX-License-Identifier: AGPL-3.0-or-later
//! Proposal logic shared by the API and push handling.
use crate::{access::Project, app::AppState, error::ApiResult};

/// A design option got new versions: proposals from it follow its head.
pub async fn on_option_moved(_state: &AppState, _project: &Project, _option: &str, _head: &str) -> ApiResult<()> {
    Ok(())
}
