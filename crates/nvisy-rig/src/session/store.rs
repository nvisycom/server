//! Session storage backed by NATS KV.
//!
//! This module provides session persistence using the NATS KV store
//! from nvisy-nats. Sessions are automatically expired based on TTL.

use std::time::Duration;

use nvisy_nats::NatsClient;
use uuid::Uuid;

use super::Session;
use crate::Result;

/// Session store backed by NATS KV.
///
/// This is a thin wrapper around `nvisy_nats::kv::SessionStore<Session>`
/// that provides session persistence for rig agents.
///
/// This type is cheap to clone and can be shared across threads.
#[derive(Clone)]
pub struct SessionStore {
    inner: nvisy_nats::kv::SessionStore<Session>,
}

impl SessionStore {
    /// Creates a new session store with default TTL (30 minutes).
    ///
    /// # Arguments
    /// * `nats` - NATS client for KV operations
    pub async fn new(nats: &NatsClient) -> Result<Self> {
        let inner = nats
            .session_store(None)
            .await
            .map_err(|e| crate::Error::session(format!("failed to create session store: {e}")))?;

        Ok(Self { inner })
    }

    /// Creates a session store with custom TTL.
    ///
    /// # Arguments
    /// * `nats` - NATS client for KV operations
    /// * `ttl` - Time-to-live for sessions
    pub async fn with_ttl(nats: &NatsClient, ttl: Duration) -> Result<Self> {
        let inner = nats
            .session_store(Some(ttl))
            .await
            .map_err(|e| crate::Error::session(format!("failed to create session store: {e}")))?;

        Ok(Self { inner })
    }

    /// Returns the bucket name.
    pub fn bucket(&self) -> &str {
        self.inner.bucket()
    }

    /// Returns the session TTL.
    pub fn ttl(&self) -> Duration {
        self.inner.ttl()
    }

    /// Creates a new session in the store.
    pub async fn create(&self, session: &Session) -> Result<()> {
        self.inner
            .create(session.id(), session)
            .await
            .map_err(|e| crate::Error::session(format!("failed to create session: {e}")))
    }

    /// Gets a session by ID.
    ///
    /// Returns `None` if the session doesn't exist or has expired.
    pub async fn get(&self, session_id: Uuid) -> Result<Option<Session>> {
        self.inner
            .get(session_id)
            .await
            .map_err(|e| crate::Error::session(format!("failed to get session: {e}")))
    }

    /// Updates an existing session.
    ///
    /// Also resets the session's TTL.
    pub async fn update(&self, session: &Session) -> Result<()> {
        self.inner
            .update(session.id(), session)
            .await
            .map_err(|e| crate::Error::session(format!("failed to update session: {e}")))
    }

    /// Touches a session to reset its TTL.
    pub async fn touch(&self, session_id: Uuid) -> Result<()> {
        self.inner
            .touch(session_id)
            .await
            .map_err(|e| crate::Error::session(format!("failed to touch session: {e}")))
    }

    /// Deletes a session.
    pub async fn delete(&self, session_id: Uuid) -> Result<()> {
        self.inner
            .delete(session_id)
            .await
            .map_err(|e| crate::Error::session(format!("failed to delete session: {e}")))
    }

    /// Lists all active sessions.
    pub async fn list(&self) -> Result<Vec<Session>> {
        self.inner
            .list()
            .await
            .map_err(|e| crate::Error::session(format!("failed to list sessions: {e}")))
    }

    /// Returns the number of active sessions.
    pub async fn len(&self) -> Result<usize> {
        self.inner
            .len()
            .await
            .map_err(|e| crate::Error::session(format!("failed to count sessions: {e}")))
    }

    /// Returns true if there are no active sessions.
    pub async fn is_empty(&self) -> Result<bool> {
        self.inner
            .is_empty()
            .await
            .map_err(|e| crate::Error::session(format!("failed to check if empty: {e}")))
    }
}
