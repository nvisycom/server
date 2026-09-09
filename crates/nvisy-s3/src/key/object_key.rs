//! The [`ObjectKey`] trait and the encoding helpers its key types share.

use std::fmt;
use std::str::FromStr;

use base64::prelude::*;
use uuid::Uuid;

use super::bucket::Bucket;
use crate::error::{Error, Result};

/// Trait for object storage keys.
///
/// Keys must be convertible to/from strings for storage addressing. Each key
/// type has a prefix that organizes objects by type within its store, and names
/// the [`Bucket`] it belongs to — so the store derives the target bucket from
/// the key alone and a key can never be written to the wrong store.
pub trait ObjectKey: fmt::Display + FromStr<Err = Error> + Clone + Send + Sync + 'static {
    /// The prefix for this key type (e.g. `file_`, `account_`).
    const PREFIX: &'static str;

    /// The logical store this key addresses.
    const BUCKET: Bucket;
}

/// Builds a key-parse error from any displayable cause.
pub(super) fn parse_error(message: impl std::fmt::Display) -> Error {
    Error::operation_msg("parse_key", message.to_string())
}

/// Strips a key type's prefix, erroring if it is absent.
pub(super) fn strip_prefix<K: ObjectKey>(s: &str) -> Result<&str> {
    s.strip_prefix(K::PREFIX)
        .ok_or_else(|| parse_error(format!("Invalid key prefix: expected '{}'", K::PREFIX)))
}

/// Splits a prefix-stripped `{id}_{version}` payload into its two parts.
pub(super) fn split_id_version<K: ObjectKey>(s: &str) -> Result<(&str, &str)> {
    strip_prefix::<K>(s)?
        .split_once('_')
        .ok_or_else(|| parse_error(format!("Expected '{}{{id}}_{{version}}'", K::PREFIX)))
}

/// Encodes two UUIDs (32 bytes) as URL-safe base64.
pub(super) fn encode_ids(first: Uuid, second: Uuid) -> String {
    let mut bytes = [0u8; 32];
    bytes[..16].copy_from_slice(first.as_bytes());
    bytes[16..].copy_from_slice(second.as_bytes());
    BASE64_URL_SAFE_NO_PAD.encode(bytes)
}

/// Decodes a URL-safe base64 payload back into two UUIDs.
pub(super) fn decode_ids(s: &str) -> Result<(Uuid, Uuid)> {
    let bytes = BASE64_URL_SAFE_NO_PAD
        .decode(s)
        .map_err(|e| parse_error(format!("Invalid base64 encoding: {e}")))?;

    if bytes.len() != 32 {
        return Err(parse_error(format!(
            "Invalid key length: expected 32 bytes, got {}",
            bytes.len()
        )));
    }

    let first = Uuid::from_slice(&bytes[..16])
        .map_err(|e| parse_error(format!("Invalid first UUID: {e}")))?;
    let second = Uuid::from_slice(&bytes[16..])
        .map_err(|e| parse_error(format!("Invalid second UUID: {e}")))?;
    Ok((first, second))
}

#[cfg(test)]
mod tests {
    use base64::prelude::*;
    use uuid::Uuid;

    use super::{decode_ids, encode_ids, split_id_version};
    use crate::key::document_key::FileKey;

    #[test]
    fn encode_decode_round_trips_two_uuids() {
        let a = Uuid::now_v7();
        let b = Uuid::now_v7();
        // The 32-byte payload is exactly the two UUIDs, order preserved.
        let (first, second) = decode_ids(&encode_ids(a, b)).unwrap();
        assert_eq!((first, second), (a, b));
    }

    #[test]
    fn decode_ids_rejects_bad_base64() {
        // `*` is not in the URL-safe base64 alphabet.
        assert!(decode_ids("not*base64").is_err());
    }

    #[test]
    fn decode_ids_rejects_a_payload_that_is_not_32_bytes() {
        // Valid base64, but decodes to 16 bytes (one UUID) rather than 32.
        let short = BASE64_URL_SAFE_NO_PAD.encode(Uuid::now_v7().as_bytes());
        assert!(decode_ids(&short).is_err());
    }

    #[test]
    fn split_id_version_requires_the_underscore_separator() {
        // Prefix present but no `{id}_{version}` separator after it.
        assert!(split_id_version::<FileKey>("file_noseparator").is_err());
        // A well-formed payload splits into its two parts.
        let (id, version) = split_id_version::<FileKey>("file_abc_v2").unwrap();
        assert_eq!((id, version), ("abc", "v2"));
    }
}
