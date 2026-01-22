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

    /// Failed to construct credentials registry.
    #[error("failed to construct credentials registry: {0}")]
    CredentialsRegistry(#[source] serde_json::Error),

    /// Credentials not found.
    #[error("credentials not found: {0}")]
    CredentialsNotFound(Uuid),

    /// Storage operation failed.
    #[error("storage error: {0}")]
    Storage(#[from] nvisy_dal::StorageError),

    /// Serialization/deserialization error.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Internal error.
    #[error("internal error: {0}")]
    Internal(String),
}
