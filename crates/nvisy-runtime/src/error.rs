//! Workflow error types.

use thiserror::Error;
use uuid::Uuid;

use crate::definition::NodeId;

/// Result type for workflow operations.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Errors that can occur during workflow operations.
#[derive(Debug, Error)]
pub enum Error {
    /// Workflow definition is invalid.
    #[error("invalid workflow definition: {0}")]
    InvalidDefinition(String),

    /// Node configuration is invalid.
    #[error("invalid config for node {node_id}: {message}")]
    InvalidNodeConfig {
        /// ID of the node with invalid config.
        node_id: NodeId,
        /// Error message.
        message: String,
    },

    /// Node execution failed.
    #[error("node {node_id} failed: {message}")]
    NodeFailed {
        /// ID of the failed node.
        node_id: NodeId,
        /// Error message.
        message: String,
    },

    /// Workflow execution was cancelled.
    #[error("workflow execution cancelled")]
    Cancelled,

    /// Workflow execution timed out.
    #[error("workflow execution timed out")]
    Timeout,

    /// Failed to construct connection registry.
    #[error("failed to construct connection registry: {0}")]
    ConnectionRegistry(#[source] serde_json::Error),

    /// Connection not found.
    #[error("connection not found: {0}")]
    ConnectionNotFound(Uuid),

    /// Storage operation failed.
    #[error("storage error: {0}")]
    Storage(#[from] nvisy_dal::Error),

    /// Serialization/deserialization error.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Internal error.
    #[error("internal error: {0}")]
    Internal(String),
}

impl Error {
    /// Creates an error from a PostgreSQL error.
    pub fn from_postgres(err: nvisy_postgres::PgError) -> Self {
        Self::Internal(format!("database error: {err}"))
    }
}
