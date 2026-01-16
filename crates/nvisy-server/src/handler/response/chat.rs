//! Chat session response types.

use jiff::Timestamp;
use nvisy_postgres::model;
use nvisy_postgres::types::ChatSessionStatus;
use nvisy_rig::chat::ChatEvent;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::Page;

/// Represents a chat session with full details.
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChatSession {
    /// Unique session identifier.
    pub session_id: Uuid,
    /// ID of the workspace this session belongs to.
    pub workspace_id: Uuid,
    /// ID of the account that owns this session.
    pub account_id: Uuid,
    /// ID of the primary file being edited.
    pub primary_file_id: Uuid,
    /// Display name of the session.
    pub display_name: String,
    /// Current session status.
    pub session_status: ChatSessionStatus,
    /// LLM configuration.
    pub model_config: serde_json::Value,
    /// Total number of messages in this session.
    pub message_count: i32,
    /// Total tokens used in this session.
    pub token_count: i32,
    /// Timestamp when the session was created.
    pub created_at: Timestamp,
    /// Timestamp when the session was last updated.
    pub updated_at: Timestamp,
}

impl ChatSession {
    /// Creates a response from a database model.
    pub fn from_model(session: model::ChatSession) -> Self {
        Self {
            session_id: session.id,
            workspace_id: session.workspace_id,
            account_id: session.account_id,
            primary_file_id: session.primary_file_id,
            display_name: session.display_name,
            session_status: session.session_status,
            model_config: session.model_config,
            message_count: session.message_count,
            token_count: session.token_count,
            created_at: session.created_at.into(),
            updated_at: session.updated_at.into(),
        }
    }
}

/// Paginated list of chat sessions.
pub type ChatSessionsPage = Page<ChatSession>;

/// SSE event wrapper for chat streaming.
///
/// This wraps `ChatEvent` from nvisy-rig and provides SSE-compatible serialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatStreamEvent {
    /// The underlying chat event.
    #[serde(flatten)]
    pub event: ChatEvent,
}

impl ChatStreamEvent {
    /// Creates a new stream event from a chat event.
    pub fn new(event: ChatEvent) -> Self {
        Self { event }
    }

    /// Returns the SSE event type name.
    pub fn event_type(&self) -> &'static str {
        match &self.event {
            ChatEvent::Thinking { .. } => "thinking",
            ChatEvent::TextDelta { .. } => "text_delta",
            ChatEvent::ToolCall { .. } => "tool_call",
            ChatEvent::ToolResult { .. } => "tool_result",
            ChatEvent::ProposedEdit { .. } => "proposed_edit",
            ChatEvent::EditApplied { .. } => "edit_applied",
            ChatEvent::Done { .. } => "done",
            ChatEvent::Error { .. } => "error",
        }
    }
}
