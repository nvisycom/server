//! Chat tool call model for PostgreSQL database operations.
//!
//! This module provides models for tracking tool invocations within chat sessions.
//! Tool calls represent individual operations performed by the LLM, such as
//! merging, splitting, redacting, or translating document content.
//!
//! ## Models
//!
//! - [`ChatToolCall`] - Main tool call model with execution details
//! - [`NewChatToolCall`] - Data structure for creating new tool calls
//! - [`UpdateChatToolCall`] - Data structure for updating existing tool calls

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::chat_tool_calls;
use crate::types::{ChatToolStatus, HasCreatedAt};

/// Chat tool call model representing a tool invocation within a session.
///
/// This model tracks individual tool calls made during editing sessions,
/// including the tool name, input parameters, output results, and execution
/// status. Tool calls are linked to specific files and optionally to chunks.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = chat_tool_calls)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct ChatToolCall {
    /// Unique tool call identifier.
    pub id: Uuid,
    /// Reference to the chat session this tool call belongs to.
    pub session_id: Uuid,
    /// Reference to the file being operated on.
    pub file_id: Uuid,
    /// Optional reference to a specific chunk within the file.
    pub chunk_id: Option<Uuid>,
    /// Name of the tool being invoked.
    pub tool_name: String,
    /// Tool input parameters as JSON.
    pub tool_input: serde_json::Value,
    /// Tool output results as JSON.
    pub tool_output: serde_json::Value,
    /// Current execution status of the tool call.
    pub tool_status: ChatToolStatus,
    /// Timestamp when the tool call was created/started.
    pub started_at: Timestamp,
    /// Timestamp when the tool execution completed.
    pub completed_at: Option<Timestamp>,
}

/// Data structure for creating a new chat tool call.
///
/// Contains all the information necessary to record a new tool invocation.
/// The tool status defaults to pending, and output is populated upon completion.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = chat_tool_calls)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewChatToolCall {
    /// Reference to the chat session.
    pub session_id: Uuid,
    /// Reference to the file being operated on.
    pub file_id: Uuid,
    /// Optional reference to a specific chunk.
    pub chunk_id: Option<Uuid>,
    /// Name of the tool being invoked.
    pub tool_name: String,
    /// Tool input parameters as JSON.
    pub tool_input: Option<serde_json::Value>,
    /// Optional initial tool output.
    pub tool_output: Option<serde_json::Value>,
    /// Optional initial tool status.
    pub tool_status: Option<ChatToolStatus>,
}

/// Data structure for updating an existing chat tool call.
///
/// Contains optional fields for modifying tool call properties. Primarily
/// used to update the status and output upon completion or cancellation.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = chat_tool_calls)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UpdateChatToolCall {
    /// Updated tool output results.
    pub tool_output: Option<serde_json::Value>,
    /// Updated execution status.
    pub tool_status: Option<ChatToolStatus>,
    /// Updated completion timestamp.
    pub completed_at: Option<Option<Timestamp>>,
}

impl ChatToolCall {
    /// Returns whether the tool call is pending execution.
    #[inline]
    pub fn is_pending(&self) -> bool {
        self.tool_status.is_pending()
    }

    /// Returns whether the tool is currently running.
    #[inline]
    pub fn is_running(&self) -> bool {
        self.tool_status.is_running()
    }

    /// Returns whether the tool execution completed successfully.
    #[inline]
    pub fn is_completed(&self) -> bool {
        self.tool_status.is_completed()
    }

    /// Returns whether the tool execution was cancelled.
    #[inline]
    pub fn is_cancelled(&self) -> bool {
        self.tool_status.is_cancelled()
    }

    /// Returns whether the tool is in a final state.
    #[inline]
    pub fn is_final(&self) -> bool {
        self.tool_status.is_final()
    }

    /// Returns whether the tool call targets a specific chunk.
    #[inline]
    pub fn has_chunk(&self) -> bool {
        self.chunk_id.is_some()
    }

    /// Returns whether the tool has input parameters.
    pub fn has_input(&self) -> bool {
        !self.tool_input.as_object().is_none_or(|obj| obj.is_empty())
    }

    /// Returns whether the tool has output results.
    pub fn has_output(&self) -> bool {
        !self
            .tool_output
            .as_object()
            .is_none_or(|obj| obj.is_empty())
    }

    /// Returns the execution duration if the tool has completed.
    pub fn execution_duration(&self) -> Option<jiff::Span> {
        self.completed_at.map(|completed| {
            let started: jiff::Timestamp = self.started_at.into();
            let completed: jiff::Timestamp = completed.into();
            completed.since(started).unwrap_or_default()
        })
    }
}

impl HasCreatedAt for ChatToolCall {
    fn created_at(&self) -> jiff::Timestamp {
        self.started_at.into()
    }
}
