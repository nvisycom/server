//! The in-flight OAuth authorization-state bucket and its key.

use std::marker::PhantomData;
use std::time::Duration;

use derive_more::{Display, From, FromStr};
use serde::Serialize;
use serde::de::DeserializeOwned;

use super::core::{KvBucket, KvKey};

/// The CSRF-state key for an in-flight OAuth authorization. The value is the
/// opaque state token the provider echoes back to the callback; NATS KV keys
/// allow the URL-safe characters an OAuth state uses.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Display, FromStr, From)]
pub struct OAuthStateKey(pub String);

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oauth_state_key_roundtrip() {
        let key = OAuthStateKey("abc-123_XYZ".to_owned());
        let parsed: OAuthStateKey = key.to_string().parse().unwrap();
        assert_eq!(key, parsed);
    }
}
