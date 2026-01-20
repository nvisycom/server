//! Session storage backed by NATS KV.
//!
//! This module provides session persistence using the NATS KV store
//! from nvisy-nats. Sessions are automatically expired based on TTL.

use std::time::Duration;

use derive_more::{Deref, DerefMut};
use nvisy_nats::NatsClient;
use nvisy_nats::kv::{ChatHistoryBucket, KvStore, SessionKey};

use super::Session;
use crate::Result;

/// Type alias for session KV store.
type SessionKvStore = KvStore<SessionKey, Session, ChatHistoryBucket>;

/// Session store backed by NATS KV.
///
/// This is a thin wrapper around `KvStore<SessionKey, Session, ChatHistoryBucket>`
/// that provides session persistence for rig agents.
///
/// This type is cheap to clone and can be shared across threads.
#[derive(Clone, Deref, DerefMut)]
pub struct SessionStore {
    #[deref]
    #[deref_mut]
    inner: SessionKvStore,
}

impl SessionStore {
    /// Creates a new session store with default TTL (30 minutes).
    pub async fn new(nats: NatsClient) -> Result<Self> {
        let inner = nats
            .chat_history_store()
            .await
            .map_err(|e| crate::Error::session(format!("failed to create store: {e}")))?;
        Ok(Self { inner })
    }

    /// Creates a session store with custom TTL.
    pub async fn with_ttl(nats: NatsClient, ttl: Duration) -> Result<Self> {
        let inner = nats
            .chat_history_store_with_ttl(ttl)
            .await
            .map_err(|e| crate::Error::session(format!("failed to create store: {e}")))?;
        Ok(Self { inner })
    }

    /// Creates a new session.
    pub async fn create(&self, session: &Session) -> Result<()> {
        let key = SessionKey::from(session.id());
        self.inner
            .put(&key, session)
            .await
            .map_err(|e| crate::Error::session(format!("failed to create: {e}")))?;
        Ok(())
    }

    /// Gets a session by ID.
    pub async fn get(&self, session_id: uuid::Uuid) -> Result<Option<Session>> {
        let key = SessionKey::from(session_id);
        self.inner
            .get_value(&key)
            .await
            .map_err(|e| crate::Error::session(format!("failed to get: {e}")))
    }

    /// Updates an existing session (also resets TTL).
    pub async fn update(&self, session: &Session) -> Result<()> {
        let key = SessionKey::from(session.id());
        self.inner
            .put(&key, session)
            .await
            .map_err(|e| crate::Error::session(format!("failed to update: {e}")))?;
        Ok(())
    }

    /// Touches a session to reset its TTL.
    pub async fn touch(&self, session_id: uuid::Uuid) -> Result<()> {
        let key = SessionKey::from(session_id);
        self.inner
            .touch(&key)
            .await
            .map_err(|e| crate::Error::session(format!("failed to touch: {e}")))?;
        Ok(())
    }

    /// Deletes a session.
    pub async fn delete(&self, session_id: uuid::Uuid) -> Result<()> {
        let key = SessionKey::from(session_id);
        self.inner
            .delete(&key)
            .await
            .map_err(|e| crate::Error::session(format!("failed to delete: {e}")))
    }
}
