// SPDX-License-Identifier: AGPL-3.0-or-later
//! JSON API routes.
use crate::app::AppState;
use axum::Router;

pub mod auth;
pub mod orgs;

pub fn router() -> Router<AppState> {
    Router::new().merge(auth::router()).merge(orgs::router())
}
