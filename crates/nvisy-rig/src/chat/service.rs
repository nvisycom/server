//! Chat service for managing sessions and conversations.

use std::sync::Arc;

use nvisy_nats::NatsClient;
use uuid::Uuid;

use super::ChatStream;
use crate::Result;
use crate::provider::{CompletionModel, EmbeddingProvider};
use crate::session::{CreateSession, Session, SessionStore};
use crate::tool::ToolRegistry;
use crate::tool::edit::ApplyResult;

/// Inner state for [`ChatService`].
struct ChatServiceInner {
    embedding_provider: EmbeddingProvider,
    tools: ToolRegistry,
    sessions: SessionStore,
}

/// Chat service for AI-powered document conversations.
#[derive(Clone)]
pub struct ChatService {
    inner: Arc<ChatServiceInner>,
}

impl ChatService {
    /// Creates a new ChatService.
    pub async fn new(embedding_provider: EmbeddingProvider, nats: NatsClient) -> Result<Self> {
        let tools = ToolRegistry::with_defaults();
        let sessions = SessionStore::new(nats).await?;

        Ok(Self {
            inner: Arc::new(ChatServiceInner {
                embedding_provider,
                tools,
                sessions,
            }),
        })
    }

    /// Creates a new ChatService with custom tools and session store.
    pub fn with_components(
        embedding_provider: EmbeddingProvider,
        tools: ToolRegistry,
        sessions: SessionStore,
    ) -> Self {
        Self {
            inner: Arc::new(ChatServiceInner {
                embedding_provider,
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
    pub async fn chat(&self, session_id: Uuid, message: &str) -> Result<ChatStream> {
        self.inner.sessions.touch(session_id).await?;

        let session = self
            .inner
            .sessions
            .get(session_id)
            .await?
            .ok_or_else(|| crate::Error::session("session not found"))?;

        ChatStream::new(session, message.to_string(), self.clone()).await
    }

    /// Sends a chat message with a specific model override.
    pub async fn chat_with_model(
        &self,
        session_id: Uuid,
        message: &str,
        model: CompletionModel,
    ) -> Result<ChatStream> {
        self.inner.sessions.touch(session_id).await?;

        let session = self
            .inner
            .sessions
            .get(session_id)
            .await?
            .ok_or_else(|| crate::Error::session("session not found"))?;

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
    pub async fn embed(&self, text: &str) -> Result<Vec<f64>> {
        let embedding = self.inner.embedding_provider.embed_text(text).await?;
        Ok(embedding.vec)
    }

    /// Returns a reference to the embedding provider.
    pub fn embedding_provider(&self) -> &EmbeddingProvider {
        &self.inner.embedding_provider
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
