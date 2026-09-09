//! The scheduler leader-election lock bucket and its key.

use std::str::FromStr;
use std::time::Duration;

use derive_more::{Display, From};

use super::core::{InvalidKvKey, KvBucket, KvKey, validate_kv_key};

/// A string key for named locks and similar coordination entries.
///
/// [`FromStr`] enforces NATS KV key syntax (via [`validate_kv_key`]), like the
/// other key types, so a malformed lock name is rejected on parse rather than at
/// the store.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Display, From)]
pub struct SchedulerLockKey(pub String);

impl FromStr for SchedulerLockKey {
    type Err = InvalidKvKey;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        validate_kv_key(s).map(Self)
    }
}

impl KvKey for SchedulerLockKey {}

/// Bucket for short-lived distributed leader-election locks.
///
/// Entries expire quickly so a crashed holder never blocks future ticks. The
/// value is a nonce; only the key's presence matters.
pub enum SchedulerLocksBucket {}

impl KvBucket for SchedulerLocksBucket {
    type Key = SchedulerLockKey;
    type Value = u64;

    const DESCRIPTION: &'static str = "Short-lived scheduler leader-election locks";
    const NAME: &'static str = "scheduler_locks";
    // Comfortably above the scheduler tick interval so a period's lock outlives
    // its period despite clock skew; per-period keys expire on their own.
    const TTL: Option<Duration> = Some(Duration::from_secs(5 * 60));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_key_roundtrip() {
        let key = SchedulerLockKey("scheduler.42".to_owned());
        let parsed: SchedulerLockKey = key.to_string().parse().unwrap();
        assert_eq!(key, parsed);
    }
}
