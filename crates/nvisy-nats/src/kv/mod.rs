//! NATS Key-Value store operations for caching, API tokens, and generic KV storage.
//!
//! This module provides type-safe abstractions over NATS KV for different use cases:
//! - `KvStore<T>`: Generic type-safe key-value operations
//! - `CacheStore<T>`: Type-safe caching with cache-aside patterns
//! - `ApiTokenStore`: API authentication token management
//!
//! All stores provide compile-time type safety through generic parameters and
//! comprehensive observability through structured logging.

mod api_token;
mod api_token_store;
mod cache;
mod store;

// Re-export core KV types
// Re-export cache functionality
// Re-export API token functionality for convenience
pub use api_token::{ApiToken, ApiTokenType, TokenStoreStats};
pub use api_token_store::ApiTokenStore;
pub use cache::{CacheStats, CacheStore};
pub use store::{KvEntry, KvStore, KvValue};
