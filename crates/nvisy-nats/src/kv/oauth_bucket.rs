//! The in-flight OAuth authorization-state bucket and its key.

use std::marker::PhantomData;
use std::str::FromStr;
use std::time::Duration;

use derive_more::Display;
use serde::Serialize;
use serde::de::DeserializeOwned;

use super::core::{InvalidKvKey, KvBucket, KvKey, validate_kv_key};

/// The CSRF-state key for an in-flight OAuth authorization. The value is the
/// opaque state token the provider echoes back to the callback.
///
/// The provider echoes this back into a callback URL, so it is untrusted on the
/// way in. [`FromStr`] enforces NATS KV key syntax — non-empty and only the
/// `-/_=.` and alphanumeric characters KV allows — so a malformed value is
/// rejected before it reaches the store (a URL-safe OAuth state always passes).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Display)]
pub struct OAuthStateKey(pub String);

impl FromStr for OAuthStateKey {
    type Err = InvalidKvKey;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        validate_kv_key(s).map(Self)
    }
}

impl KvKey for OAuthStateKey {}

/// Bucket for in-flight OAuth authorization state (CSRF token + PKCE verifier),
/// held between starting an authorization and its callback.
///
/// Its value is application-owned (the flow state a consuming crate defines), so
/// the bucket is generic over `V`; a consumer pins it with a type alias, e.g.
/// `type OAuthStateBucket = nvisy_nats::kv::OAuthStateBucket<MyFlowState>`.
/// Entries are short-lived: a user completes the consent screen within minutes,
/// and an abandoned flow's state should not linger.
///
/// The bucket is a pure type-level tag, so it is uninhabited: the `V` it pins
/// lives only in the `PhantomData`, never in a value.
pub enum OAuthStateBucket<V> {
    #[doc(hidden)]
    Never(PhantomData<fn() -> V>),
}

impl<V> KvBucket for OAuthStateBucket<V>
where
    V: Serialize + DeserializeOwned + Send + Sync + 'static,
{
    type Key = OAuthStateKey;
    type Value = V;

    const DESCRIPTION: &'static str = "In-flight OAuth authorization state";
    const NAME: &'static str = "oauth_state";
    const TTL: Option<Duration> = Some(Duration::from_secs(10 * 60)); // 10 minutes
}
