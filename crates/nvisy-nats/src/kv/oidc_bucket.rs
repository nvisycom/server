//! The in-flight OIDC sign-in state bucket and its key.
//!
//! Kept separate from the connection-OAuth bucket ([`OAuthStateBucket`]): the two
//! serve different purposes — signing a user in versus linking a cloud file
//! connector — and carry different state (an OIDC sign-in also holds a `nonce`),
//! so they get independent buckets rather than sharing one.
//!
//! [`OAuthStateBucket`]: super::OAuthStateBucket

use std::marker::PhantomData;
use std::str::FromStr;
use std::time::Duration;

use derive_more::Display;
use serde::Serialize;
use serde::de::DeserializeOwned;

use super::core::{InvalidKvKey, KvBucket, KvKey, validate_kv_key};

/// The CSRF-state key for an in-flight OIDC sign-in. The value is the opaque
/// state token the provider echoes back to the callback.
///
/// The provider echoes this into a callback URL, so it is untrusted on the way
/// in. [`FromStr`] enforces NATS KV key syntax — non-empty and only the `-/_=.`
/// and alphanumeric characters KV allows — so a malformed value is rejected
/// before it reaches the store (a URL-safe OAuth state always passes).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Display)]
pub struct OidcStateKey(pub String);

impl FromStr for OidcStateKey {
    type Err = InvalidKvKey;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        validate_kv_key(s).map(Self)
    }
}

impl KvKey for OidcStateKey {}

/// Bucket for in-flight OIDC sign-in state (CSRF token, PKCE verifier, and
/// nonce), held between starting a sign-in and its callback.
///
/// Its value is application-owned (the flow state the server defines), so the
/// bucket is generic over `V`; a consumer pins it with a type alias, e.g.
/// `type OidcStateBucket = nvisy_nats::kv::OidcStateBucket<MyFlowState>`. Entries
/// are short-lived: a user completes the provider's consent screen within
/// minutes, and an abandoned sign-in's state should not linger.
///
/// The bucket is a pure type-level tag, so it is uninhabited: the `V` it pins
/// lives only in the `PhantomData`, never in a value.
pub enum OidcStateBucket<V> {
    #[doc(hidden)]
    Never(PhantomData<fn() -> V>),
}

impl<V> KvBucket for OidcStateBucket<V>
where
    V: Serialize + DeserializeOwned + Send + Sync + 'static,
{
    type Key = OidcStateKey;
    type Value = V;

    const DESCRIPTION: &'static str = "In-flight OIDC sign-in state";
    const NAME: &'static str = "oidc_state";
    const TTL: Option<Duration> = Some(Duration::from_secs(10 * 60)); // 10 minutes
}
