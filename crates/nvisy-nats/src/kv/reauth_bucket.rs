//! The step-up re-authentication proof bucket and its key.
//!
//! A *reauth proof* is minted when an authenticated user completes a fresh OIDC
//! sign-in with a provider already linked to their account, and is then required
//! to authorize a credential-adding action (setting a first password, linking a
//! new provider). It exists so those actions need current control of a linked
//! identity, not merely a live session — a stolen session alone cannot mint a
//! durable new credential.
//!
//! The proof is short-lived and single-use: the consuming handler reads it and
//! deletes it, so it authorizes exactly one action within a small window.

use std::marker::PhantomData;
use std::str::FromStr;
use std::time::Duration;

use derive_more::Display;
use serde::Serialize;
use serde::de::DeserializeOwned;

use super::core::{KvBucket, KvKey};

/// The key of a step-up re-authentication proof: an opaque, unguessable id minted
/// by the reauth callback and presented by the credential-adding action.
///
/// It reaches the server in a request body, so it is untrusted on the way in.
/// [`FromStr`] enforces NATS KV key syntax — non-empty and only the `-/_=.` and
/// alphanumeric characters KV allows — so a malformed value is rejected before it
/// reaches the store.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Display)]
pub struct ReauthProofKey(pub String);

impl FromStr for ReauthProofKey {
    type Err = InvalidReauthProofKey;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            return Err(InvalidReauthProofKey);
        }
        let valid = s
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'/' | b'_' | b'=' | b'.'));
        if valid {
            Ok(Self(s.to_owned()))
        } else {
            Err(InvalidReauthProofKey)
        }
    }
}

/// Returned when a string is not a valid [`ReauthProofKey`] (empty, or containing
/// characters NATS KV keys disallow).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("invalid reauth proof key")]
pub struct InvalidReauthProofKey;

impl KvKey for ReauthProofKey {}

/// Bucket for step-up re-authentication proofs.
///
/// Its value is application-owned (what the proof attests, e.g. the account it is
/// good for), so the bucket is generic over `V`; a consumer pins it with a type
/// alias. Proofs are short-lived: a step-up action follows its re-auth within a
/// few minutes, and an unused proof should not linger as a standing capability.
///
/// The bucket is a pure type-level tag, so it is uninhabited: the `V` it pins
/// lives only in the `PhantomData`, never in a value.
pub enum ReauthProofBucket<V> {
    #[doc(hidden)]
    Never(PhantomData<fn() -> V>),
}

impl<V> KvBucket for ReauthProofBucket<V>
where
    V: Serialize + DeserializeOwned + Send + Sync + 'static,
{
    type Key = ReauthProofKey;
    type Value = V;

    const DESCRIPTION: &'static str = "Step-up re-authentication proofs";
    const NAME: &'static str = "reauth_proof";
    const TTL: Option<Duration> = Some(Duration::from_secs(5 * 60)); // 5 minutes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reauth_proof_key_roundtrip() {
        let key = ReauthProofKey("abc-123_XYZ=".to_owned());
        let parsed: ReauthProofKey = key.to_string().parse().unwrap();
        assert_eq!(key, parsed);
    }

    #[test]
    fn reauth_proof_key_rejects_invalid() {
        assert!("".parse::<ReauthProofKey>().is_err());
        assert!("has space".parse::<ReauthProofKey>().is_err());
        assert!("bad*char".parse::<ReauthProofKey>().is_err());
    }
}
