//! Chat message model for PostgreSQL database operations.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::chat_messages;
use crate::types::ChatRole;

/// One message in a chat session.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = chat_messages)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct ChatMessage {
    /// Unique message identifier.
    pub id: Uuid,
    /// Session this message belongs to.
    pub session_id: Uuid,
    /// Parent in the conversation tree; `None` is a root.
    pub parent_id: Option<Uuid>,
    /// Author of the message.
    pub role: ChatRole,
    /// Message text, XChaCha20-Poly1305 encrypted with the workspace key.
    pub content: Vec<u8>,
    /// Message creation timestamp.
    pub created_at: Timestamp,
}

/// Data for appending a new chat message.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = chat_messages)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewChatMessage {
    /// Session this message belongs to.
    pub session_id: Uuid,
    /// Parent in the conversation tree; `None` is a root.
    pub parent_id: Option<Uuid>,
    /// Author of the message.
    pub role: ChatRole,
    /// Message text, XChaCha20-Poly1305 encrypted with the workspace key.
    pub content: Vec<u8>,
}
