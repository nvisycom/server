//! NATS Key-Value store operations.
//!
//! A [`KvBucket`] fixes a store's value type, name, and entry TTL, and every
//! entry is keyed by a [`TokenKey`], so a [`KvStore`] is selected by its bucket
//! alone. Buckets are defined where their value lives, with
//! [`kv_bucket!`](crate::kv_bucket), and created at startup with
//! [`NatsClient::ensure_kv_bucket`](crate::NatsClient::ensure_kv_bucket).
//!
//! # Example
//!
//! ```ignore
//! let store = nats_client.kv_store::<MyBucket>().await?;
//! store.put(&key, &value).await?;
//! let value = store.take(&key).await?;
//! ```

mod core;
mod typed_store;

pub use core::{InvalidKvKey, KvBucket, TokenKey};

pub use typed_store::KvStore;
