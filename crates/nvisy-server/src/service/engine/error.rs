//! Engine service error types.

use thiserror::Error;

/// A file-filter token that matched no known extension or modality.
#[derive(Debug, Clone, Error)]
pub enum UnknownFormatToken {
    /// An extension that resolves to no registered format.
    #[error("unknown file extension: {0}")]
    Extension(String),
    /// A modality keyword that matches no registered format's modality.
    #[error("unknown modality: {0}")]
    Modality(String),
}
