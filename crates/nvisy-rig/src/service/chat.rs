//! Chat types and streaming response handling.

use std::pin::Pin;
use std::task::{Context, Poll};

use futures::Stream;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::provider::ModelRef;
use super::{RigService, UsageStats};
use crate::Result;
use crate::session::Session;
use crate::tool::edit::ProposedEdit;
use crate::tool::{ToolCall, ToolResult};

/// Events emitted during chat processing.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChatEvent {
    /// Agent is thinking/planning.
    Thinking { content: String },

    /// Text delta from the model.
    TextDelta { delta: String },

    /// Agent is calling a tool.
    ToolCall { call: ToolCall },

    /// Tool execution completed.
    ToolResult { result: ToolResult },

    /// Agent proposes an edit to the document.
    ProposedEdit { edit: ProposedEdit },

    /// Edit was auto-applied based on policy.
    EditApplied { edit_id: Uuid },

    /// Chat response completed.
    Done { response: ChatResponse },

    /// Error occurred during processing.
    Error { message: String },
}

/// Complete chat response after stream ends.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    /// Unique message ID.
    pub id: Uuid,

    /// Complete response text.
    pub content: String,

    /// Model used for completion.
    pub model: String,

    /// Token usage statistics.
    pub usage: UsageStats,

    /// Proposed edits from this response.
    pub proposed_edits: Vec<ProposedEdit>,

    /// Edits that were auto-applied.
    pub applied_edits: Vec<Uuid>,
}

impl ChatResponse {
    /// Creates a new chat response.
    pub fn new(content: String, model: String, usage: UsageStats) -> Self {
        Self {
            id: Uuid::now_v7(),
            content,
            model,
            usage,
            proposed_edits: Vec::new(),
            applied_edits: Vec::new(),
        }
    }

    /// Adds proposed edits to the response.
    pub fn with_proposed_edits(mut self, edits: Vec<ProposedEdit>) -> Self {
        self.proposed_edits = edits;
        self
    }

    /// Adds applied edits to the response.
    pub fn with_applied_edits(mut self, edit_ids: Vec<Uuid>) -> Self {
        self.applied_edits = edit_ids;
        self
    }
}

/// Streaming chat response.
///
/// Implements `Stream<Item = Result<ChatEvent>>` for async iteration.
pub struct ChatStream {
    session: Session,
    message: String,
    model_override: Option<ModelRef>,
    service: RigService,

    // State
    started: bool,
    finished: bool,
    accumulated_content: String,
    proposed_edits: Vec<ProposedEdit>,
    applied_edits: Vec<Uuid>,
}

impl ChatStream {
    /// Creates a new chat stream.
    pub async fn new(session: Session, message: String, service: RigService) -> Result<Self> {
        Ok(Self {
            session,
            message,
            model_override: None,
            service,
            started: false,
            finished: false,
            accumulated_content: String::new(),
            proposed_edits: Vec::new(),
            applied_edits: Vec::new(),
        })
    }

    /// Creates a new chat stream with a model override.
    pub async fn with_model(
        session: Session,
        message: String,
        model_override: Option<ModelRef>,
        service: RigService,
    ) -> Result<Self> {
        Ok(Self {
            session,
            message,
            model_override,
            service,
            started: false,
            finished: false,
            accumulated_content: String::new(),
            proposed_edits: Vec::new(),
            applied_edits: Vec::new(),
        })
    }

    /// Returns the session ID.
    pub fn session_id(&self) -> Uuid {
        self.session.id()
    }

    /// Returns the document ID being processed.
    pub fn document_id(&self) -> Uuid {
        self.session.document_id()
    }

    /// Polls the underlying agent for the next event.
    fn poll_next_event(&mut self, _cx: &mut Context<'_>) -> Poll<Option<Result<ChatEvent>>> {
        if self.finished {
            return Poll::Ready(None);
        }

        if !self.started {
            self.started = true;

            // TODO: Start the actual agent pipeline:
            // 1. Retrieve relevant context via RAG
            // 2. Build prompt with tools, context, and history
            // 3. Stream completion from provider
            // 4. Handle tool calls and proposed edits
            // 5. Apply auto-apply policies

            // For now, emit a placeholder response
            // These references silence unused warnings until the pipeline is implemented
            let _ = (&self.message, &self.service, &self.accumulated_content);

            // Emit done event with placeholder
            self.finished = true;

            let model = self
                .model_override
                .as_ref()
                .map(|m| m.to_string())
                .unwrap_or_else(|| "default".to_string());

            let response = ChatResponse::new(
                "Agent pipeline not yet implemented".to_string(),
                model,
                UsageStats::default(),
            )
            .with_proposed_edits(self.proposed_edits.clone())
            .with_applied_edits(self.applied_edits.clone());

            return Poll::Ready(Some(Ok(ChatEvent::Done { response })));
        }

        Poll::Ready(None)
    }
}

impl Stream for ChatStream {
    type Item = Result<ChatEvent>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.poll_next_event(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_event_serialization() {
        let event = ChatEvent::TextDelta {
            delta: "Hello".to_string(),
        };

        let json = serde_json::to_string(&event).expect("ChatEvent should serialize to JSON");
        assert!(json.contains("text_delta"));
        assert!(json.contains("Hello"));
    }

    #[test]
    fn chat_response_builder() {
        let response = ChatResponse::new(
            "Test content".to_string(),
            "gpt-4".to_string(),
            UsageStats::default(),
        );

        assert!(!response.id.is_nil());
        assert_eq!(response.content, "Test content");
        assert_eq!(response.model, "gpt-4");
    }
}
