//! The core KV contracts: the entry key and the bucket trait every store is
//! built on.

use std::str::FromStr;
use std::time::Duration;

use derive_more::Display;
use serde::Serialize;
use serde::de::DeserializeOwned;

/// Returned when a string is not a valid NATS KV key (empty, or containing
/// characters KV keys disallow).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("invalid KV key")]
pub struct InvalidKvKey;

/// A KV entry key: an opaque token (an OAuth or OIDC state, a reauth proof id)
/// or a lock name.
///
/// Keys often arrive untrusted (a state echoed into a callback URL, a proof in a
/// request body), so [`FromStr`] is the only constructor and enforces NATS KV key
/// syntax: non-empty, only alphanumeric and `-/_=.` characters, and no empty
/// dot-separated token. A URL-safe base64 token always passes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[derive(Display)]
pub struct TokenKey(String);

impl FromStr for TokenKey {
    type Err = InvalidKvKey;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let chars_valid = value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'/' | b'_' | b'=' | b'.'));
        // `.` separates NATS subject tokens, so no token may be empty (no
        // leading, trailing, or consecutive dot); this also rejects "".
        let tokens_nonempty = value.split('.').all(|token| !token.is_empty());
        if chars_valid && tokens_nonempty {
            Ok(Self(value.to_owned()))
        } else {
            Err(InvalidKvKey)
        }
    }
}

/// A NATS KV bucket: the value it stores and its name, description, and entry
/// TTL. A store is selected by its bucket alone; every bucket is keyed by
/// [`TokenKey`].
///
/// A bucket is a pure type-level tag, never instantiated. Define one with
/// [`kv_bucket!`](crate::kv_bucket).
pub trait KvBucket: 'static {
    /// The value type stored in this bucket.
    type Value: Serialize + DeserializeOwned + Send + Sync + 'static;

    /// Bucket name used in NATS KV.
    const NAME: &'static str;

    /// Human-readable description for the bucket.
    const DESCRIPTION: &'static str;

    /// How long an entry lives before it expires.
    const TTL: Duration;
}

/// Defines a [`KvBucket`]: an uninhabited tag type with its value, name,
/// description, and entry TTL.
///
/// ```ignore
/// nvisy_nats::kv_bucket! {
///     /// In-flight OAuth authorization state.
///     pub(crate) OAuthStateBucket: OAuthFlowState {
///         name: "oauth_state",
///         description: "In-flight OAuth authorization state",
///         ttl: Duration::from_mins(10),
///     }
/// }
/// ```
#[macro_export]
macro_rules! kv_bucket {
    (
        $(#[$meta:meta])*
        $vis:vis $bucket:ident: $value:ty {
            name: $name:literal,
            description: $description:literal,
            ttl: $ttl:expr $(,)?
        }
    ) => {
        $(#[$meta])*
        $vis enum $bucket {}

        impl $crate::kv::KvBucket for $bucket {
            type Value = $value;

            const DESCRIPTION: &'static str = $description;
            const NAME: &'static str = $name;
            const TTL: ::std::time::Duration = $ttl;
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(value: &str) -> Result<String, InvalidKvKey> {
        value.parse::<TokenKey>().map(|key| key.to_string())
    }

    #[test]
    fn accepts_url_safe_tokens_unchanged() {
        assert_eq!(parse("abc-123_XYZ=").unwrap(), "abc-123_XYZ=");
        assert_eq!(parse("a/b.c").unwrap(), "a/b.c");
    }

    #[test]
    fn rejects_empty_and_disallowed_chars() {
        for value in ["", "has space", "bad*char", "newline\n"] {
            assert_eq!(parse(value), Err(InvalidKvKey), "{value:?}");
        }
    }

    #[test]
    fn rejects_empty_dot_tokens() {
        for value in [".", ".a", "a.", "a..b"] {
            assert_eq!(parse(value), Err(InvalidKvKey), "{value:?}");
        }
        assert_eq!(parse("a.b").unwrap(), "a.b");
    }
}
