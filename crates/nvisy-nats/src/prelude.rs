//! Prelude module for nvisy-nats.
//!
//! This module re-exports the most commonly used types and traits from nvisy-nats,
//! making it easy to import everything you need with a single `use` statement.
//!
//! # Example
//!
//! ```rust,ignore
//! use nvisy_nats::prelude::*;
//!
//! # async fn example() -> Result<()> {
//! let config = NatsConfig::new("nats://localhost:4222", "my-token");
//! let client = NatsClient::connect(config).await?;
//! # Ok(())
//! # }
//! ```

// Client types
// Re-export ObjectInfo for convenience
pub use async_nats::jetstream::object_store::ObjectInfo;

pub use crate::client::{NatsClient, NatsConfig, NatsConnection};
// Key-Value store types
pub use crate::kv::{
    ApiToken, ApiTokenStore, ApiTokenType, CacheStats, CacheStore, KvEntry, KvStore, KvValue,
    TokenStoreStats,
};
// Object store types
pub use crate::object::{
    DocumentBucket, DocumentKey, DocumentStore, Files, GetResult, Intermediates, ObjectStore,
    PutResult,
};
// Stream types
pub use crate::stream::{
    DocumentJob, DocumentJobPublisher, DocumentJobSubscriber, EventPriority, StreamPublisher,
    StreamSubscriber, WorkspaceEvent, WorkspaceEventPublisher, WorkspaceEventSubscriber,
};
// Error types
pub use crate::{Error, Result};
