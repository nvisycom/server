//! NATS Key-Value store operations.
//!
//! The [`KvKey`] and [`KvBucket`] contracts back the generic [`KvStore`], which
//! is selected by a bucket alone. Concrete buckets and their keys are grouped by
//! domain (scheduler locks, OAuth state).
//!
//! # Example
//!
//! ```ignore
//! // A bucket fixes its key and value types, so the store is selected by the
//! // bucket alone.
//! let store = nats_client.kv_store::<MyBucket>().await?;
//! store.put(&key, &value).await?;
//! let value = store.get_value(&key).await?;
//! ```

mod core;
mod oauth_bucket;
mod oidc_bucket;
mod reauth_bucket;
mod scheduler_bucket;
mod typed_store;

pub use core::{InvalidKvKey, KvBucket, KvKey, validate_kv_key};

pub use oauth_bucket::{OAuthStateBucket, OAuthStateKey};
pub use oidc_bucket::{OidcStateBucket, OidcStateKey};
pub use reauth_bucket::{ReauthProofBucket, ReauthProofKey};
pub use scheduler_bucket::{SchedulerLockKey, SchedulerLocksBucket};
pub use typed_store::{KvEntry, KvStore, KvValue};
