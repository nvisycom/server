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
