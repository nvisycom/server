//! Chat operation model for PostgreSQL database operations.
//!
//! This module provides models for tracking document operations (diffs) produced
//! by tool calls. Operations represent the actual changes to be applied to documents,
//! supporting apply/revert functionality for undo capabilities.
//!
//! ## Models
//!
//! - [`ChatOperation`] - Main operation model with diff details
//! - [`NewChatOperation`] - Data structure for creating new operations
//! - [`UpdateChatOperation`] - Data structure for updating existing operations

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::chat_operations;
use crate::types::HasCreatedAt;

/// Chat operation model representing a document operation (diff).
///
/// This model tracks individual operations produced by tool calls that can be
/// applied to or reverted from documents. Operations store position-based diffs
/// rather than content, enabling efficient undo/redo functionality.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = chat_operations)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct ChatOperation {
    /// Unique operation identifier.
    pub id: Uuid,
    /// Reference to the tool call that produced this operation.
    pub tool_call_id: Uuid,
    /// Reference to the file being modified.
    pub file_id: Uuid,
    /// Optional reference to a specific chunk within the file.
    pub chunk_id: Option<Uuid>,
    /// Type of operation (insert, replace, delete, format, merge, split, etc.).
    pub operation_type: String,
    /// The diff specification as JSON (positions, not content).
    pub operation_diff: serde_json::Value,
    /// Whether this operation has been applied to the document.
    pub applied: bool,
    /// Whether this operation was reverted by the user.
    pub reverted: bool,
    /// Timestamp when the operation was created.
    pub created_at: Timestamp,
    /// Timestamp when the operation was applied.
    pub applied_at: Option<Timestamp>,
}

/// Data structure for creating a new chat operation.
///
/// Contains all the information necessary to record a new document operation.
/// Operations are created as unapplied by default and can be applied later.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = chat_operations)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewChatOperation {
    /// Reference to the tool call that produced this operation.
    pub tool_call_id: Uuid,
    /// Reference to the file being modified.
    pub file_id: Uuid,
    /// Optional reference to a specific chunk.
    pub chunk_id: Option<Uuid>,
    /// Type of operation.
    pub operation_type: String,
    /// The diff specification as JSON.
    pub operation_diff: Option<serde_json::Value>,
    /// Optional initial applied state.
    pub applied: Option<bool>,
    /// Optional initial reverted state.
    pub reverted: Option<bool>,
}

/// Data structure for updating an existing chat operation.
///
/// Contains optional fields for modifying operation properties. Primarily
/// used to mark operations as applied or reverted.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = chat_operations)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UpdateChatOperation {
    /// Updated applied state.
    pub applied: Option<bool>,
    /// Updated reverted state.
    pub reverted: Option<bool>,
    /// Updated applied timestamp.
    pub applied_at: Option<Option<Timestamp>>,
}

impl ChatOperation {
    /// Returns whether the operation has been applied.
    #[inline]
    pub fn is_applied(&self) -> bool {
        self.applied
    }

    /// Returns whether the operation has been reverted.
    #[inline]
    pub fn is_reverted(&self) -> bool {
        self.reverted
    }

    /// Returns whether the operation is pending (not yet applied).
    #[inline]
    pub fn is_pending(&self) -> bool {
        !self.applied
    }

    /// Returns whether the operation can be applied.
    #[inline]
    pub fn can_apply(&self) -> bool {
        !self.applied
    }

    /// Returns whether the operation can be reverted.
    #[inline]
    pub fn can_revert(&self) -> bool {
        self.applied && !self.reverted
    }

    /// Returns whether the operation targets a specific chunk.
    #[inline]
    pub fn has_chunk(&self) -> bool {
        self.chunk_id.is_some()
    }

    /// Returns whether the operation has diff data.
    pub fn has_diff(&self) -> bool {
        !self
            .operation_diff
            .as_object()
            .is_none_or(|obj| obj.is_empty())
    }

    /// Returns the time between creation and application, if applied.
    pub fn time_to_apply(&self) -> Option<jiff::Span> {
        self.applied_at.map(|applied| {
            let created: jiff::Timestamp = self.created_at.into();
            let applied: jiff::Timestamp = applied.into();
            applied.since(created).unwrap_or_default()
        })
    }

    /// Returns whether this is an insert operation.
    #[inline]
    pub fn is_insert(&self) -> bool {
        self.operation_type == "insert"
    }

    /// Returns whether this is a replace operation.
    #[inline]
    pub fn is_replace(&self) -> bool {
        self.operation_type == "replace"
    }

    /// Returns whether this is a delete operation.
    #[inline]
    pub fn is_delete(&self) -> bool {
        self.operation_type == "delete"
    }

    /// Returns whether this is a format operation.
    #[inline]
    pub fn is_format(&self) -> bool {
        self.operation_type == "format"
    }

    /// Returns whether this is a merge operation.
    #[inline]
    pub fn is_merge(&self) -> bool {
        self.operation_type == "merge"
    }

    /// Returns whether this is a split operation.
    #[inline]
    pub fn is_split(&self) -> bool {
        self.operation_type == "split"
    }
}

impl HasCreatedAt for ChatOperation {
    fn created_at(&self) -> jiff::Timestamp {
        self.created_at.into()
    }
}
