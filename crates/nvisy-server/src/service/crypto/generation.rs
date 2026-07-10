//! Random secret generation.

use rand::Rng;

/// Bytes of entropy in a generated secret token.
const SECRET_SIZE: usize = 32;

/// Generates a fresh [`SECRET_SIZE`]-byte random secret, hex-encoded.
pub fn generate_secret() -> String {
    let mut bytes = [0u8; SECRET_SIZE];
    rand::rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}
