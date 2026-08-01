//! Engine service error types.

use std::fmt;

/// A file-filter token that matched no known extension or modality.
#[derive(Debug, Clone)]
pub enum UnknownFormatToken {
    /// An extension that resolves to no registered format.
    Extension(String),
    /// A modality keyword that matches no registered format's modality.
    Modality(String),
}

impl fmt::Display for UnknownFormatToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Extension(t) => write!(f, "unknown file extension: {t}"),
            Self::Modality(t) => write!(f, "unknown modality: {t}"),
        }
    }
}
