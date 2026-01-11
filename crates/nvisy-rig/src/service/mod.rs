//! High-level service for AI-powered document processing.
//!
//! This module provides:
//! - [`RigService`] - Main entry point for nvisy-server
//! - [`ChatStream`] - Streaming chat response
//! - [`ChatEvent`] - Events emitted during chat
//! - [`provider`] - Multi-provider management

mod chat;
pub mod provider;
mod stream;

use std::sync::Arc;

pub use chat::{ChatEvent, ChatResponse, ChatStream};
pub use provider::{ModelRef, ProviderRegistry};
pub use stream::UsageStats;
use uuid::Uuid;

use crate::Result;
// Re-export session types for convenience
pub use crate::session::ApplyPolicy;
use crate::session::{CreateSession, Session, SessionStore};
use crate::tool::ToolRegistry;
use crate::tool::edit::ApplyResult;

/// Inner state for [`RigService`].
struct RigServiceInner {
    providers: ProviderRegistry,
    tools: ToolRegistry,
    sessions: SessionStore,
}

/// Main entry point for AI-powered document processing.
///
/// This type is cheap to clone and can be shared across threads.
///
/// Provides a high-level API for:
/// - Creating and managing chat sessions
/// - Streaming chat responses with tool use
/// - Approving and applying document edits
#[derive(Clone)]
pub struct RigService {
    inner: Arc<RigServiceInner>,
}

impl RigService {
    /// Creates a new RigService.
    pub fn new(providers: ProviderRegistry, tools: ToolRegistry, sessions: SessionStore) -> Self {
        Self {
            inner: Arc::new(RigServiceInner {
                providers,
                tools,
                sessions,
            }),
        }
    }

    /// Creates a new chat session for a document.
    pub async fn create_session(&self, request: CreateSession) -> Result<Session> {
        let session = Session::new(request);
        self.inner.sessions.create(&session).await?;
        Ok(session)
    }

    /// Retrieves an existing session.
    pub async fn get_session(&self, session_id: Uuid) -> Result<Option<Session>> {
        self.inner.sessions.get(session_id).await
    }

    /// Sends a chat message and returns a streaming response.
    ///
    /// The stream emits [`ChatEvent`]s as the agent processes the request,
    /// including thinking, tool calls, proposed edits, and text deltas.
    pub async fn chat(&self, session_id: Uuid, message: &str) -> Result<ChatStream> {
        // Touch session to reset TTL
        self.inner.sessions.touch(session_id).await?;

        // Get session
        let session = self
            .inner
            .sessions
            .get(session_id)
            .await?
            .ok_or_else(|| crate::Error::session("session not found"))?;

        // Create chat stream
        ChatStream::new(session, message.to_string(), self.clone()).await
    }

    /// Sends a chat message with a specific model override.
    pub async fn chat_with_model(
        &self,
        session_id: Uuid,
        message: &str,
        model: ModelRef,
    ) -> Result<ChatStream> {
        // Touch session to reset TTL
        self.inner.sessions.touch(session_id).await?;

        // Get session
        let session = self
            .inner
            .sessions
            .get(session_id)
            .await?
            .ok_or_else(|| crate::Error::session("session not found"))?;

        // Create chat stream with model override
        ChatStream::with_model(session, message.to_string(), Some(model), self.clone()).await
    }

    /// Approves and applies pending edits.
    pub async fn apply_edits(&self, session_id: Uuid, edit_ids: &[Uuid]) -> Result<ApplyResult> {
        let mut session = self
            .inner
            .sessions
            .get(session_id)
            .await?
            .ok_or_else(|| crate::Error::session("session not found"))?;

        let result = session.apply_edits(edit_ids)?;
        self.inner.sessions.update(&session).await?;

        Ok(result)
    }

    /// Rejects pending edits.
    pub async fn reject_edits(&self, session_id: Uuid, edit_ids: &[Uuid]) -> Result<()> {
        let mut session = self
            .inner
            .sessions
            .get(session_id)
            .await?
            .ok_or_else(|| crate::Error::session("session not found"))?;

        session.reject_edits(edit_ids);
        self.inner.sessions.update(&session).await?;

        Ok(())
    }

    /// Ends a session and cleans up all pending edits.
    pub async fn end_session(&self, session_id: Uuid) -> Result<()> {
        self.inner.sessions.delete(session_id).await
    }

    /// Generates embeddings for text.
    ///
    /// Used for indexing documents into the vector store.
    pub async fn embed(&self, text: &str, model: Option<&ModelRef>) -> Result<Vec<f32>> {
        let (_provider, _model_name) = self.inner.providers.resolve_embedding(model)?;

        // TODO: Implement using rig-core embedding
        let _ = text;
        Err(crate::Error::provider(
            "rig",
            "embedding not yet implemented",
        ))
    }

    /// Returns a reference to the provider registry.
    pub fn providers(&self) -> &ProviderRegistry {
        &self.inner.providers
    }

    /// Returns a reference to the tool registry.
    pub fn tools(&self) -> &ToolRegistry {
        &self.inner.tools
    }

    /// Returns a reference to the session store.
    pub fn sessions(&self) -> &SessionStore {
        &self.inner.sessions
    }
}
