//! Agent context for a single request.

use crate::provider::estimate_tokens;
use crate::rag::RetrievedChunk;
use crate::session::Session;

/// Context for an agent request.
#[derive(Debug, Clone)]
pub struct AgentContext {
    /// The session this request belongs to.
    session: Session,

    /// The user's message.
    message: String,

    /// Retrieved document chunks for RAG.
    retrieved_chunks: Vec<RetrievedChunk>,
}

impl AgentContext {
    /// Creates a new agent context.
    pub fn new(session: Session, message: String, retrieved_chunks: Vec<RetrievedChunk>) -> Self {
        Self {
            session,
            message,
            retrieved_chunks,
        }
    }

    /// Returns the session.
    pub fn session(&self) -> &Session {
        &self.session
    }

    /// Returns the user's message.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns the retrieved chunks.
    pub fn retrieved_chunks(&self) -> &[RetrievedChunk] {
        &self.retrieved_chunks
    }

    /// Returns whether there are any retrieved chunks.
    pub fn has_context(&self) -> bool {
        !self.retrieved_chunks.is_empty()
    }

    /// Returns the total token count of retrieved chunks (estimated).
    pub fn context_tokens(&self) -> u32 {
        self.retrieved_chunks
            .iter()
            .filter_map(|c| c.content.as_deref())
            .map(estimate_tokens)
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;
    use crate::session::CreateSession;

    #[test]
    fn context_without_chunks() {
        let session = Session::new(CreateSession::new(
            Uuid::now_v7(),
            Uuid::now_v7(),
            Uuid::now_v7(),
        ));
        let context = AgentContext::new(session, "Hello".to_string(), Vec::new());

        assert!(!context.has_context());
        assert_eq!(context.context_tokens(), 0);
    }
}
