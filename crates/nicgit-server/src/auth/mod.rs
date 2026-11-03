// SPDX-License-Identifier: AGPL-3.0-or-later
//! Passwords, session and personal-access tokens. Secrets are stored only as hashes.
mod extract;
pub use extract::{CurrentUser, MaybeUser, SESSION_COOKIE};

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use rand::{distributions::Alphanumeric, Rng, RngCore};
use sha2::{Digest, Sha256};

pub const PAT_PREFIX: &str = "ngp_";
pub const SESSION_DAYS: i64 = 30;
pub const MIN_PASSWORD: usize = 10;

/// A real hash to verify against when the user does not exist, so a missing name costs the same time.
pub static DUMMY_HASH: std::sync::LazyLock<String> =
    std::sync::LazyLock::new(|| hash_password("nicgit-dummy-password").expect("argon2 works"));

pub fn hash_password(password: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    Ok(Argon2::default().hash_password(password.as_bytes(), &salt).map_err(|e| anyhow::anyhow!("{e}"))?.to_string())
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    PasswordHash::new(hash).is_ok_and(|h| Argon2::default().verify_password(password.as_bytes(), &h).is_ok())
}

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
