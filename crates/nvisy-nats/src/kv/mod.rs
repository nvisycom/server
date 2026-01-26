//! NATS Key-Value store operations.
//!
//! This module provides type-safe abstractions over NATS KV:
//! - `KvStore<K, V, B>`: Generic type-safe key-value operations
//! - `KvKey`: Trait for key types with prefix support
//! - `KvBucket`: Trait for bucket configuration
//!
//! # Example
//!
//! ```ignore
//! // Create a session store
//! let store: KvStore<SessionKey, MySession, ChatHistoryBucket> =
//!     nats_client.kv_store().await?;
//!
//! // Put a session
//! let key = SessionKey::from(Uuid::new_v4());
//! store.put(&key, &session).await?;
//!
//! // Get the session back
//! let session = store.get_value(&key).await?;
//! ```

mod api_token;
mod kv_bucket;
mod kv_key;
mod kv_store;

pub use api_token::{ApiToken, ApiTokenType};
pub use kv_bucket::{ApiTokensBucket, ChatHistoryBucket, KvBucket};
pub use kv_key::{KvKey, SessionKey, TokenKey};
pub use kv_store::{KvEntry, KvStore, KvValue};
