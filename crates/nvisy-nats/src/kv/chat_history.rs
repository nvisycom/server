//! Chat history store for ephemeral sessions with TTL.

use std::time::Duration;

use async_nats::jetstream;
use derive_more::{Deref, DerefMut};
use serde::Serialize;
use serde::de::DeserializeOwned;
use uuid::Uuid;

use super::KvStore;
use crate::{Result, TRACING_TARGET_KV};

/// Default session TTL (30 minutes).
pub const DEFAULT_SESSION_TTL: Duration = Duration::from_secs(30 * 60);

/// NATS KV bucket name for chat history.
const CHAT_HISTORY_BUCKET: &str = "chat_history";

/// Chat history store backed by NATS KV.
///
/// Provides ephemeral session storage with automatic TTL expiration.
#[derive(Clone, Deref, DerefMut)]
pub struct ChatHistoryStore<T>
where
    T: Serialize + DeserializeOwned + Send + Sync + 'static,
{
    #[deref]
    #[deref_mut]
    store: KvStore<T>,
    ttl: Duration,
}

impl<T> ChatHistoryStore<T>
where
    T: Serialize + DeserializeOwned + Send + Sync + 'static,
{
    /// Creates a new chat history store with default TTL (30 minutes).
    #[tracing::instrument(skip(jetstream), target = TRACING_TARGET_KV)]
    pub async fn new(jetstream: &jetstream::Context) -> Result<Self> {
        Self::with_ttl(jetstream, DEFAULT_SESSION_TTL).await
    }

    /// Creates a new chat history store with custom TTL.
    #[tracing::instrument(skip(jetstream), target = TRACING_TARGET_KV)]
    pub async fn with_ttl(jetstream: &jetstream::Context, ttl: Duration) -> Result<Self> {
        let store = KvStore::new(
            jetstream,
            CHAT_HISTORY_BUCKET,
            Some("Ephemeral chat sessions"),
            Some(ttl),
        )
        .await?;

        tracing::info!(
            target: TRACING_TARGET_KV,
            ttl_secs = ttl.as_secs(),
            bucket = %store.bucket_name(),
            "Created chat history store"
        );

        Ok(Self { store, ttl })
    }

    /// Returns the configured TTL.
    pub fn ttl(&self) -> Duration {
        self.ttl
    }

    /// Creates a new session.
    #[tracing::instrument(skip(self, session), target = TRACING_TARGET_KV)]
    pub async fn create(&self, session_id: Uuid, session: &T) -> Result<()> {
        let key = session_key(session_id);

        if self.store.exists(&key).await? {
            return Err(crate::Error::operation(
                "chat_history_create",
                format!("session already exists: {session_id}"),
            ));
        }

        self.store.put(&key, session).await?;

        tracing::info!(
            target: TRACING_TARGET_KV,
            session_id = %session_id,
            "Created chat session"
        );

        Ok(())
    }

    /// Gets a session by ID.
    #[tracing::instrument(skip(self), target = TRACING_TARGET_KV)]
    pub async fn get(&self, session_id: Uuid) -> Result<Option<T>> {
        let key = session_key(session_id);
        self.store.get_value(&key).await
    }

    /// Updates an existing session (also resets TTL).
    #[tracing::instrument(skip(self, session), target = TRACING_TARGET_KV)]
    pub async fn update(&self, session_id: Uuid, session: &T) -> Result<()> {
        let key = session_key(session_id);

        if !self.store.exists(&key).await? {
            return Err(crate::Error::operation(
                "chat_history_update",
                format!("session not found: {session_id}"),
            ));
        }

        self.store.put(&key, session).await?;

        tracing::debug!(
            target: TRACING_TARGET_KV,
            session_id = %session_id,
            "Updated chat session"
        );

        Ok(())
    }

    /// Touches a session to reset its TTL.
    #[tracing::instrument(skip(self), target = TRACING_TARGET_KV)]
    pub async fn touch(&self, session_id: Uuid) -> Result<()> {
        let key = session_key(session_id);
        self.store.touch(&key).await?;

        tracing::debug!(
            target: TRACING_TARGET_KV,
            session_id = %session_id,
            "Touched chat session"
        );

        Ok(())
    }

    /// Deletes a session.
    #[tracing::instrument(skip(self), target = TRACING_TARGET_KV)]
    pub async fn delete(&self, session_id: Uuid) -> Result<()> {
        let key = session_key(session_id);
        self.store.delete(&key).await?;

        tracing::info!(
            target: TRACING_TARGET_KV,
            session_id = %session_id,
            "Deleted chat session"
        );

        Ok(())
    }
}

fn session_key(session_id: Uuid) -> String {
    format!("session.{session_id}")
}
