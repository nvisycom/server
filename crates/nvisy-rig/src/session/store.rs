//! Session storage backed by NATS KV.
//!
//! This module provides session persistence using the NATS KV store
//! from nvisy-nats. Sessions are automatically expired based on TTL.

use std::time::Duration;

use derive_more::{Deref, DerefMut};
use nvisy_nats::NatsClient;
use nvisy_nats::kv::ChatHistoryStore;

use super::Session;
use crate::Result;

/// Session store backed by NATS KV.
///
/// This is a thin wrapper around `nvisy_nats::kv::ChatHistoryStore<Session>`
/// that provides session persistence for rig agents.
///
/// This type is cheap to clone and can be shared across threads.
#[derive(Clone, Deref, DerefMut)]
pub struct SessionStore {
    #[deref]
    #[deref_mut]
    inner: ChatHistoryStore<Session>,
}

impl SessionStore {
    /// Creates a new session store with default TTL (30 minutes).
    pub async fn new(nats: NatsClient) -> Result<Self> {
        let inner = nats
            .chat_history_store(None)
            .await
            .map_err(|e| crate::Error::session(format!("failed to create store: {e}")))?;
        Ok(Self { inner })
    }

    /// Creates a session store with custom TTL.
    pub async fn with_ttl(nats: NatsClient, ttl: Duration) -> Result<Self> {
        let inner = nats
            .chat_history_store(Some(ttl))
            .await
            .map_err(|e| crate::Error::session(format!("failed to create store: {e}")))?;
        Ok(Self { inner })
    }

    /// Creates a new session.
    pub async fn create(&self, session: &Session) -> Result<()> {
        self.inner
            .create(session.id(), session)
            .await
            .map_err(|e| crate::Error::session(format!("failed to create: {e}")))
    }

    /// Gets a session by ID.
    pub async fn get(&self, session_id: uuid::Uuid) -> Result<Option<Session>> {
        self.inner
            .get(session_id)
            .await
            .map_err(|e| crate::Error::session(format!("failed to get: {e}")))
    }

    /// Updates an existing session (also resets TTL).
    pub async fn update(&self, session: &Session) -> Result<()> {
        self.inner
            .update(session.id(), session)
            .await
            .map_err(|e| crate::Error::session(format!("failed to update: {e}")))
    }

    /// Touches a session to reset its TTL.
    pub async fn touch(&self, session_id: uuid::Uuid) -> Result<()> {
        self.inner
            .touch(session_id)
            .await
            .map_err(|e| crate::Error::session(format!("failed to touch: {e}")))
    }

    /// Deletes a session.
    pub async fn delete(&self, session_id: uuid::Uuid) -> Result<()> {
        self.inner
            .delete(session_id)
            .await
            .map_err(|e| crate::Error::session(format!("failed to delete: {e}")))
    }
}
