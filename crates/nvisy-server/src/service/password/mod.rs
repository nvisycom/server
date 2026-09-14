//! Password handling: hashing, verification, and strength policy.
//!
//! [`PasswordService`] is the public entry point; it composes an Argon2
//! [`PasswordHasher`] with a [`PasswordStrength`] policy.
//!
//! [`PasswordHasher`]: hasher::PasswordHasher
//! [`PasswordStrength`]: strength::PasswordStrength

mod hasher;
mod service;
mod strength;

pub use service::PasswordService;
