// SPDX-License-Identifier: AGPL-3.0-or-later
//! JSON API routes.
use crate::app::AppState;
use axum::Router;

pub mod auth;
pub mod inbox;
pub mod internal;
pub mod issues;
pub mod jobs;
pub mod milestones;
pub mod orgs;
pub mod projects;
pub mod proposals;
pub mod rules;
pub mod transfer;
pub mod versions;
pub mod views;

pub fn router(max_upload_mb: u64) -> Router<AppState> {
    Router::new()
        .merge(auth::router())
        .merge(orgs::router())
        .merge(projects::router())
        .merge(versions::router())
        .merge(internal::router())
        .merge(jobs::router())
        .merge(views::router())
        .merge(proposals::router())
        .merge(rules::router())
        .merge(issues::router())
        .merge(milestones::router())
        .merge(transfer::router(max_upload_mb))
        .merge(inbox::router())
}
