// SPDX-License-Identifier: AGPL-3.0-or-later
//! JSON API routes.
use crate::app::AppState;
use axum::Router;

pub mod accounts;
pub mod auth;
pub mod ci;
pub mod google;
pub mod home;
pub mod inbox;
pub mod internal;
pub mod issues;
pub mod job_files;
pub mod jobs;
pub mod milestones;
pub mod orgs;
pub mod projects;
pub mod proposals;
pub mod rules;
pub mod transfer;
pub mod versions;
pub mod views;

/// What engine workers use. Served on its own address, which is never published (decision N35).
pub fn internal_router() -> Router<AppState> {
    Router::new().merge(internal::router()).merge(job_files::router())
}

pub fn router(max_file_mb: u64) -> Router<AppState> {
    Router::new()
        .merge(auth::router())
        .merge(google::router())
        .merge(orgs::router())
        .merge(projects::router())
        .merge(versions::router())
        .merge(ci::router())
        .merge(jobs::router())
        .merge(views::router())
        .merge(proposals::router())
        .merge(rules::router())
        .merge(issues::router())
        .merge(milestones::router())
        .merge(transfer::router(max_file_mb))
        .merge(inbox::router())
        .merge(home::router())
        .merge(accounts::router())
}
