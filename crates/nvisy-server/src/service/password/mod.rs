//! Password handling: hashing, verification, and strength policy.
//!
//! [`PasswordService`] is the public entry point; it composes an Argon2
//! [`PasswordHasher`](hasher::PasswordHasher) with a
//! [`PasswordStrength`](strength::PasswordStrength) policy.

mod hasher;
mod service;
mod strength;

pub use service::PasswordService;
