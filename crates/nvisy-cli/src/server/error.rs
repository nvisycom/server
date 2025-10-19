//! Server error types.

use std::io;

use thiserror::Error;

/// Result type for server operations.
pub type Result<T> = std::result::Result<T, ServerError>;

/// Error type for server startup and lifecycle operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ServerError {
    /// Server configuration is invalid.
    #[error("Invalid server configuration: {0}")]
    InvalidConfig(String),

    /// Failed to bind to the specified address.
    #[error("Failed to bind to address {address}: {source}")]
    BindError {
        /// The address that failed to bind.
        address: String,
        /// The underlying I/O error.
        #[source]
        source: io::Error,
    },

    /// Server encountered an error during operation.
    #[error("Server runtime error: {0}")]
    Runtime(#[from] io::Error),

    /// TLS configuration error.
    ///
    /// Only available when the `tls` feature is enabled.
    #[cfg(feature = "tls")]
    #[error("TLS configuration error: {0}")]
    TlsConfig(String),

    /// TLS certificate error.
    ///
    /// Only available when the `tls` feature is enabled.
    #[cfg(feature = "tls")]
    #[error("TLS certificate error: {0}")]
    TlsCertificate(String),
}
