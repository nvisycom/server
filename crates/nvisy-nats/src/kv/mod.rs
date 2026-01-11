//! NATS Key-Value store operations for caching, API tokens, chat history, and generic KV storage.
//!
//! This module provides type-safe abstractions over NATS KV for different use cases:
//! - `KvStore<T>`: Generic type-safe key-value operations
//! - `CacheStore<T>`: Type-safe caching with cache-aside patterns
//! - `ApiTokenStore`: API authentication token management
//! - `ChatHistoryStore<T>`: Ephemeral chat session storage with TTL
//!
//! All stores provide compile-time type safety through generic parameters and
//! comprehensive observability through structured logging.

mod api_token;
mod api_token_store;
mod cache;
mod chat_history;
mod store;

pub use api_token::{ApiToken, ApiTokenType};
pub use api_token_store::ApiTokenStore;
pub use cache::{CacheStats, CacheStore};
pub use chat_history::{ChatHistoryStore, DEFAULT_SESSION_TTL};
pub use store::{KvEntry, KvStore, KvValue};
