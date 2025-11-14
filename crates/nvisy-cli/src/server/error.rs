//! Server error types with enhanced context and recovery suggestions.

use std::io;

use thiserror::Error;

/// Result type for server operations.
pub type ServerResult<T> = std::result::Result<T, ServerError>;

/// Comprehensive error type for server operations with detailed context and recovery suggestions.
#[derive(Debug, Error)]
pub enum ServerError {
    /// Server configuration is invalid.
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    /// Failed to bind to the specified address.
    #[error("Failed to bind to {address}: {source}")]
    #[allow(dead_code)]
    BindError {
        address: String,
        #[source]
        source: io::Error,
    },

    /// Runtime server error.
    #[error("Runtime error: {0}")]
    Runtime(#[source] io::Error),

    /// TLS configuration error.
    #[error("TLS certificate error: {0}")]
    #[allow(dead_code)]
    TlsCertificate(String),
}

impl ServerError {
    /// Creates an invalid configuration error from an anyhow error.
    pub fn invalid_config(err: &anyhow::Error) -> Self {
        Self::InvalidConfig(err.to_string())
    }

    /// Creates a bind error with address context.
    #[allow(dead_code)]
    pub fn bind_error(address: &str, source: io::Error) -> Self {
        Self::BindError {
            address: address.to_string(),
            source,
        }
    }

    /// Returns a unique error code for this error type.
    pub const fn error_code(&self) -> &'static str {
        match self {
            Self::InvalidConfig(_) => "E001",
            Self::BindError { .. } => "E002",
            Self::Runtime(_) => "E003",
            Self::TlsCertificate(_) => "E004",
        }
    }

    /// Determines if this error is potentially recoverable.
    ///
    /// Recoverable errors are those that might succeed if retried or
    /// if the environment changes (e.g., different port, wait for resource).
    pub fn is_recoverable(&self) -> bool {
        match self {
            Self::InvalidConfig(_) | Self::TlsCertificate(_) => false, // Need manual intervention
            Self::BindError { source, .. } => {
                match source.kind() {
                    io::ErrorKind::PermissionDenied
                    | io::ErrorKind::AddrInUse
                    | io::ErrorKind::AddrNotAvailable => true, // Can retry
                    _ => false,
                }
            }
            Self::Runtime(err) => {
                match err.kind() {
                    io::ErrorKind::PermissionDenied
                    | io::ErrorKind::Interrupted
                    | io::ErrorKind::TimedOut
                    | io::ErrorKind::ConnectionRefused => true, // Can retry
                    _ => false,
                }
            }
        }
    }

    /// Provides a human-readable suggestion for resolving this error.
    pub fn suggestion(&self) -> Option<&'static str> {
        match self {
            Self::InvalidConfig(_) => Some(
                "Check your configuration file and ensure all required fields are set correctly",
            ),
            Self::BindError { source, .. } => match source.kind() {
                io::ErrorKind::PermissionDenied => {
                    Some("Try using a port above 1024 or run with appropriate privileges")
                }
                io::ErrorKind::AddrInUse => Some(
                    "The port is already in use. Try a different port or stop the conflicting service",
                ),
                io::ErrorKind::AddrNotAvailable => {
                    Some("The address is not available. Check network interface configuration")
                }
                _ => Some("Check network configuration and firewall settings"),
            },
            Self::Runtime(err) => match err.kind() {
                io::ErrorKind::PermissionDenied => Some("Check file and network permissions"),
                io::ErrorKind::Interrupted => Some("The operation was interrupted, you may retry"),
                io::ErrorKind::TimedOut => {
                    Some("The operation timed out, consider increasing timeout values")
                }
                io::ErrorKind::ConnectionRefused => {
                    Some("Connection was refused, check if the service is running")
                }
                _ => None,
            },
            Self::TlsCertificate(_) => {
                Some("Verify certificate and key files exist and are in correct PEM format")
            }
        }
    }

    /// Determines if this is a network-related error.
    pub fn is_network_error(&self) -> bool {
        matches!(self, Self::BindError { .. })
            || matches!(self, Self::Runtime(err) if matches!(err.kind(),
                io::ErrorKind::ConnectionRefused |
                io::ErrorKind::ConnectionAborted |
                io::ErrorKind::ConnectionReset |
                io::ErrorKind::AddrInUse |
                io::ErrorKind::AddrNotAvailable
            ))
    }

    /// Returns contextual information about this error as key-value pairs.
    ///
    /// This is useful for structured logging and debugging.
    pub fn context(&self) -> Vec<(&'static str, String)> {
        let mut context = vec![("error_code", self.error_code().to_string())];

        if let Some(suggestion) = self.suggestion() {
            context.push(("suggestion", suggestion.to_string()));
        }

        context.push(("recoverable", self.is_recoverable().to_string()));
        context.push(("network_error", self.is_network_error().to_string()));

        match self {
            Self::BindError { address, source } => {
                context.push(("address", address.clone()));
                context.push(("io_error_kind", format!("{:?}", source.kind())));
            }
            Self::Runtime(err) => {
                context.push(("io_error_kind", format!("{:?}", err.kind())));
            }
            Self::InvalidConfig(msg) => {
                context.push(("config_error", msg.clone()));
            }
            Self::TlsCertificate(msg) => {
                context.push(("tls_error", msg.clone()));
            }
        }

        context
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_codes_are_unique() {
        let config_err = ServerError::InvalidConfig("test".to_string());
        let bind_err = ServerError::bind_error("127.0.0.1:80", io::Error::other("test"));
        let runtime_err = ServerError::Runtime(io::Error::other("test"));
        let tls_err = ServerError::TlsCertificate("test".to_string());

        let codes = [
            config_err.error_code(),
            bind_err.error_code(),
            runtime_err.error_code(),
            tls_err.error_code(),
        ];

        // Ensure all codes are unique
        for i in 0..codes.len() {
            for j in i + 1..codes.len() {
                assert_ne!(codes[i], codes[j], "Error codes must be unique");
            }
        }
    }

    #[test]
    fn recoverable_errors_have_suggestions() {
        let bind_err = ServerError::bind_error(
            "127.0.0.1:80",
            io::Error::new(io::ErrorKind::PermissionDenied, "permission denied"),
        );

        assert!(bind_err.is_recoverable());
        assert!(bind_err.suggestion().is_some());
    }

    #[test]
    fn non_recoverable_errors_may_have_suggestions() {
        let config_err = ServerError::InvalidConfig("invalid field".to_string());

        assert!(!config_err.is_recoverable());
        assert!(config_err.suggestion().is_some()); // Still helpful for user
    }

    #[test]
    fn context_includes_all_relevant_fields() {
        let bind_err = ServerError::bind_error(
            "127.0.0.1:80",
            io::Error::new(io::ErrorKind::PermissionDenied, "permission denied"),
        );

        let context = bind_err.context();
        let context_keys: Vec<&str> = context.iter().map(|(k, _)| *k).collect();

        assert!(context_keys.contains(&"error_code"));
        assert!(context_keys.contains(&"suggestion"));
        assert!(context_keys.contains(&"recoverable"));
        assert!(context_keys.contains(&"network_error"));
        assert!(context_keys.contains(&"address"));
        assert!(context_keys.contains(&"io_error_kind"));
    }

    #[test]
    fn network_error_classification() {
        let bind_err = ServerError::bind_error("127.0.0.1:80", io::Error::other("test"));
        let runtime_network_err =
            ServerError::Runtime(io::Error::new(io::ErrorKind::ConnectionRefused, "test"));
        let runtime_non_network_err = ServerError::Runtime(io::Error::other("test"));
        let config_err = ServerError::InvalidConfig("test".to_string());

        assert!(bind_err.is_network_error());
        assert!(runtime_network_err.is_network_error());
        assert!(!runtime_non_network_err.is_network_error());
        assert!(!config_err.is_network_error());
    }
}
