// SPDX-License-Identifier: AGPL-3.0-or-later
//! JSON API routes.
use crate::app::AppState;
use axum::Router;

pub mod auth;
pub mod internal;
pub mod jobs;
pub mod orgs;
pub mod projects;
pub mod proposals;
pub mod rules;
pub mod versions;
pub mod views;

pub fn router() -> Router<AppState> {
    Router::new().merge(auth::router()).merge(orgs::router()).merge(projects::router()).merge(versions::router()).merge(internal::router()).merge(jobs::router()).merge(views::router()).merge(proposals::router()).merge(rules::router())
}
