//! Chat session model for PostgreSQL database operations.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::chat_sessions;

/// A workspace-scoped assistant conversation thread.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = chat_sessions)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct ChatSession {
    /// Unique session identifier.
    pub id: Uuid,
    /// Workspace this session belongs to.
    pub workspace_id: Uuid,
    /// Account that opened the session.
    pub account_id: Uuid,
    /// Human-readable title, seeded from the first message.
    pub title: String,
    /// Session creation timestamp.
    pub created_at: Timestamp,
    /// Timestamp of the most recent message.
    pub updated_at: Timestamp,
    /// Soft-deletion timestamp; `None` means live.
    pub deleted_at: Option<Timestamp>,
}

/// Data for creating a new chat session.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = chat_sessions)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewChatSession {
    /// Workspace this session belongs to.
    pub workspace_id: Uuid,
    /// Account that opened the session.
    pub account_id: Uuid,
    /// Human-readable title.
    pub title: String,
}

/// Data for updating a chat session.
#[derive(Debug, Default, Clone, AsChangeset)]
#[diesel(table_name = chat_sessions)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UpdateChatSession {
    /// New title.
    pub title: Option<String>,
    /// New most-recent-activity timestamp.
    pub updated_at: Option<Timestamp>,
}
