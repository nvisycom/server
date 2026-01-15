//! Studio session model for PostgreSQL database operations.
//!
//! This module provides models for managing LLM-assisted document editing sessions.
//! Sessions track the interaction between users and AI models during document editing,
//! including message counts, token usage, and model configuration.
//!
//! ## Models
//!
//! - [`StudioSession`] - Main session model with full configuration and status
//! - [`NewStudioSession`] - Data structure for creating new sessions
//! - [`UpdateStudioSession`] - Data structure for updating existing sessions

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::studio_sessions;
use crate::types::{HasCreatedAt, HasOwnership, HasUpdatedAt, StudioSessionStatus};

/// Studio session model representing an LLM-assisted document editing session.
///
/// This model manages the lifecycle of editing sessions where users interact with
/// AI models to edit documents. Each session tracks the primary file being edited,
/// model configuration, and usage statistics like message and token counts.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = studio_sessions)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct StudioSession {
    /// Unique session identifier.
    pub id: Uuid,
    /// Reference to the workspace this session belongs to.
    pub workspace_id: Uuid,
    /// Account that created and owns this session.
    pub account_id: Uuid,
    /// Primary file being edited in this session.
    pub primary_file_id: Uuid,
    /// User-friendly session name.
    pub display_name: String,
    /// Current lifecycle status of the session.
    pub session_status: StudioSessionStatus,
    /// LLM configuration (model, temperature, max tokens, etc.).
    pub model_config: serde_json::Value,
    /// Total number of messages exchanged in this session.
    pub message_count: i32,
    /// Total tokens used in this session.
    pub token_count: i32,
    /// Timestamp when this session was created.
    pub created_at: Timestamp,
    /// Timestamp when this session was last modified.
    pub updated_at: Timestamp,
}

/// Data structure for creating a new studio session.
///
/// Contains all the information necessary to create a new editing session.
/// Most fields have sensible defaults, allowing sessions to be created with
/// minimal required information.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = studio_sessions)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewStudioSession {
    /// Reference to the workspace this session will belong to.
    pub workspace_id: Uuid,
    /// Account creating this session.
    pub account_id: Uuid,
    /// Primary file to be edited in this session.
    pub primary_file_id: Uuid,
    /// Optional user-friendly session name.
    pub display_name: Option<String>,
    /// Optional initial session status.
    pub session_status: Option<StudioSessionStatus>,
    /// Optional LLM configuration.
    pub model_config: Option<serde_json::Value>,
}

/// Data structure for updating an existing studio session.
///
/// Contains optional fields for modifying session properties. Only the
/// fields that need to be changed should be set to Some(value), while
/// unchanged fields remain None to preserve their current values.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = studio_sessions)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UpdateStudioSession {
    /// Updated session display name.
    pub display_name: Option<String>,
    /// Updated session status.
    pub session_status: Option<StudioSessionStatus>,
    /// Updated LLM configuration.
    pub model_config: Option<serde_json::Value>,
    /// Updated message count.
    pub message_count: Option<i32>,
    /// Updated token count.
    pub token_count: Option<i32>,
}

impl StudioSession {
    /// Returns whether the session is currently active.
    #[inline]
    pub fn is_active(&self) -> bool {
        self.session_status.is_active()
    }

    /// Returns whether the session is paused.
    #[inline]
    pub fn is_paused(&self) -> bool {
        self.session_status.is_paused()
    }

    /// Returns whether the session is archived.
    #[inline]
    pub fn is_archived(&self) -> bool {
        self.session_status.is_archived()
    }

    /// Returns whether the session can accept new input.
    #[inline]
    pub fn can_accept_input(&self) -> bool {
        self.session_status.can_accept_input()
    }

    /// Returns whether the session has any messages.
    #[inline]
    pub fn has_messages(&self) -> bool {
        self.message_count > 0
    }

    /// Returns whether the session has used any tokens.
    #[inline]
    pub fn has_token_usage(&self) -> bool {
        self.token_count > 0
    }

    /// Returns whether the session has model configuration.
    pub fn has_model_config(&self) -> bool {
        !self
            .model_config
            .as_object()
            .is_none_or(|obj| obj.is_empty())
    }

    /// Returns the average tokens per message, if any messages exist.
    pub fn avg_tokens_per_message(&self) -> Option<f64> {
        if self.message_count > 0 {
            Some(self.token_count as f64 / self.message_count as f64)
        } else {
            None
        }
    }
}

impl HasCreatedAt for StudioSession {
    fn created_at(&self) -> jiff::Timestamp {
        self.created_at.into()
    }
}

impl HasUpdatedAt for StudioSession {
    fn updated_at(&self) -> jiff::Timestamp {
        self.updated_at.into()
    }
}

impl HasOwnership for StudioSession {
    fn created_by(&self) -> Uuid {
        self.account_id
    }
}
