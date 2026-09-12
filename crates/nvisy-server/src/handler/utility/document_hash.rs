//! A validated document content hash for request parameters.

use std::borrow::Cow;
use std::fmt;
use std::str::FromStr;

use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Serialize};

/// Length of a SHA-256 digest in bytes.
const SHA256_LEN: usize = 32;

/// A document content hash: a SHA-256 digest carried as a 64-character hex string.
///
/// Validates on deserialization — a value that is not exactly 32 bytes of hex is
/// rejected — so a handler holding one never has to re-check it.
#[must_use]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentHash([u8; SHA256_LEN]);

impl DocumentHash {
    /// The digest as raw bytes, for matching against the stored column.
    pub fn to_bytes(&self) -> Vec<u8> {
        self.0.to_vec()
    }
}

impl FromStr for DocumentHash {
    type Err = DocumentHashError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut bytes = [0u8; SHA256_LEN];
        hex::decode_to_slice(s, &mut bytes).map_err(|_| DocumentHashError)?;
        Ok(Self(bytes))
    }
}

impl fmt::Display for DocumentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode(self.0))
    }
}

impl Serialize for DocumentHash {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for DocumentHash {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

impl JsonSchema for DocumentHash {
    fn schema_name() -> Cow<'static, str> {
        "DocumentHash".into()
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        schemars::json_schema!({
            "type": "string",
            "description": "A SHA-256 content hash as a 64-character hex string.",
            "pattern": "^[0-9a-fA-F]{64}$",
        })
    }
}

/// The `hash` value was not a valid 64-character hex SHA-256.
#[derive(Debug)]
pub struct DocumentHashError;

impl fmt::Display for DocumentHashError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "hash must be a 64-character hex SHA-256")
    }
}

impl std::error::Error for DocumentHashError {}

#[cfg(test)]
mod tests {
    use super::DocumentHash;

    #[test]
    fn parses_valid_sha256_and_round_trips() {
        let hex = "a".repeat(64);
        let hash: DocumentHash = hex.parse().expect("64 hex chars is a valid sha256");
        assert_eq!(hash.to_bytes().len(), 32);
        assert_eq!(hash.to_string(), hex);
    }

    #[test]
    fn rejects_bad_length_and_non_hex() {
        assert!("abcd".parse::<DocumentHash>().is_err(), "too short");
        assert!(
            "a".repeat(66).parse::<DocumentHash>().is_err(),
            "too long (33 bytes)"
        );
        assert!(
            "zz".repeat(32).parse::<DocumentHash>().is_err(),
            "non-hex characters"
        );
    }
}
