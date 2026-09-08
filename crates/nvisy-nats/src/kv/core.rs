//! The core KV contracts: the key and bucket traits every store is built on.

use std::fmt;
use std::str::FromStr;
use std::time::Duration;

use serde::Serialize;
use serde::de::DeserializeOwned;

/// Marker trait for KV key types.
///
/// Defines how keys are formatted for storage in NATS KV: a key must render to
/// and parse from the string form used as the entry's KV key.
pub trait KvKey: fmt::Debug + fmt::Display + FromStr + Clone + Send + Sync + 'static {}

/// Returned when a string is not a valid NATS KV key (empty, or containing
/// characters KV keys disallow). Shared by the opaque-token key types whose
/// value arrives untrusted (OAuth/OIDC state, step-up proofs).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("invalid KV key")]
pub struct InvalidKvKey;

/// Validates that `value` is a usable NATS KV key: non-empty and composed only of
/// the alphanumeric and `-/_=.` characters KV keys allow. A URL-safe base64 token
/// (the common opaque-key case) always passes.
///
/// Returns the value on success so a caller's [`FromStr`] is a one-liner:
/// `validate_kv_key(s).map(Self)`.
pub fn validate_kv_key(value: &str) -> Result<String, InvalidKvKey> {
    if value.is_empty() {
        return Err(InvalidKvKey);
    }
    let valid = value
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'/' | b'_' | b'=' | b'.'));
    if valid {
        Ok(value.to_owned())
    } else {
        Err(InvalidKvKey)
    }
}

/// Configuration for a NATS KV bucket, similar to `ObjectBucket` for object
/// stores.
///
/// A bucket fixes both the key and the value it stores, so they are associated
/// types rather than free parameters on [`kv_store`](crate::NatsClient::kv_store):
/// a store is selected by its bucket alone. A bucket is a pure type-level tag —
/// all of its configuration lives in associated types and consts — so it is
/// never instantiated and carries no value bounds.
pub trait KvBucket: 'static {
    /// The key type addressing entries in this bucket.
    type Key: KvKey;

    /// The value type stored in this bucket.
    type Value: Serialize + DeserializeOwned + Send + Sync + 'static;

    /// Bucket name used in NATS KV.
    const NAME: &'static str;

    /// Human-readable description for the bucket.
    const DESCRIPTION: &'static str;

    /// Default TTL for entries in this bucket.
    /// Returns `None` for buckets where entries should not expire.
    const TTL: Option<Duration>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_kv_key_accepts_url_safe_tokens() {
        // A URL-safe base64 value (with `-_=` and `/.`) is a valid KV key and
        // round-trips unchanged.
        assert_eq!(validate_kv_key("abc-123_XYZ=").unwrap(), "abc-123_XYZ=");
        assert_eq!(validate_kv_key("a/b.c").unwrap(), "a/b.c");
    }

    #[test]
    fn validate_kv_key_rejects_empty_and_disallowed_chars() {
        assert_eq!(validate_kv_key(""), Err(InvalidKvKey));
        assert_eq!(validate_kv_key("has space"), Err(InvalidKvKey));
        assert_eq!(validate_kv_key("bad*char"), Err(InvalidKvKey));
        assert_eq!(validate_kv_key("newline\n"), Err(InvalidKvKey));
    }
}
