//! Assistant chat response types.

use jiff::Timestamp;
use nvisy_postgres::model::{ChatMessage as ChatMessageModel, ChatSession as ChatSessionModel};
use nvisy_postgres::types::ChatRole;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::Page;
use crate::handler::Result;
use crate::service::ChatService;

/// A chat session.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChatSession {
    /// Unique session identifier.
    pub id: Uuid,
    /// Human-readable title.
    pub title: String,
    /// Active leaf of the message tree (the conversation's resume point).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_message_id: Option<Uuid>,
    /// When the session was created.
    pub created_at: Timestamp,
    /// When the session was last active.
    pub updated_at: Timestamp,
}

impl ChatSession {
    /// Builds the response from a stored session.
    pub fn from_model(session: ChatSessionModel) -> Self {
        Self {
            id: session.id,
            title: session.title,
            current_message_id: session.current_message_id,
            created_at: session.created_at.into(),
            updated_at: session.updated_at.into(),
        }
    }
}

/// Paginated list of chat sessions.
pub type ChatSessionsPage = Page<ChatSession>;

/// A single chat message.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    /// Unique message identifier.
    pub id: Uuid,
    /// Parent in the conversation tree; absent for a root.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<Uuid>,
    /// Author of the message.
    pub role: ChatRole,
    /// Message text.
    pub content: String,
    /// When the message was created.
    pub created_at: Timestamp,
}

impl ChatMessage {
    /// Builds the response from a stored message, decrypting its content under
    /// the workspace key.
    pub fn from_model(
        message: ChatMessageModel,
        workspace_id: Uuid,
        chat: &ChatService,
    ) -> Result<Self> {
        let content = chat.decrypt_content(workspace_id, &message)?;
        Ok(Self {
            id: message.id,
            parent_id: message.parent_id,
            role: message.role,
            content,
            created_at: message.created_at.into(),
        })
    }
}
