// SPDX-License-Identifier: AGPL-3.0-or-later
//! Session and personal-access tokens. Secrets are stored only as hashes.
mod extract;
pub use extract::{CurrentUser, MaybeUser, SESSION_COOKIE};

use rand::{distributions::Alphanumeric, Rng, RngCore};
use sha2::{Digest, Sha256};

pub const PAT_PREFIX: &str = "ngp_";
pub const SESSION_DAYS: i64 = 30;

/// Hex SHA-256: how session and access tokens are stored.
pub fn sha256_hex(s: &str) -> String {
    hex::encode(Sha256::digest(s.as_bytes()))
}

pub fn new_session_token() -> String {
    let mut b = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut b);
    hex::encode(b)
}

pub fn new_pat() -> String {
    let body: String = rand::thread_rng().sample_iter(&Alphanumeric).take(40).map(char::from).collect();
    format!("{PAT_PREFIX}{body}")
}
