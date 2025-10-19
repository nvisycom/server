//! Service layer error types.
//!
//! This module provides comprehensive error handling for the service layer,
//! covering configuration, database, external service, and system errors.

use std::fmt;

use thiserror::Error;

/// Result type for service operations.
pub type Result<T> = std::result::Result<T, ServiceError>;

/// Service layer error types.
///
/// These errors represent failures in the service layer, such as configuration
/// issues, database connectivity problems, external service failures, etc.
#[derive(Debug, Error)]
pub enum ServiceError {
    /// Configuration error (invalid config values, missing files, etc.).
    #[error("Configuration error: {message}")]
    Config {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Database connection or query error.
    #[error("Database error: {message}")]
    Database {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// External service error (MinIO, OpenRouter, etc.).
    #[error("External service error ({service}): {message}")]
    ExternalService {
        service: String,
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Authentication or authorization error.
    #[error("Authentication error: {message}")]
    Auth {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// File system operation error.
    #[error("File system error: {message}")]
    FileSystem {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Network or connectivity error.
    #[error("Network error: {message}")]
    Network {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Internal service error.
    #[error("Internal service error: {message}")]
    Internal {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
}

impl ServiceError {
    /// Creates a new configuration error.
    pub fn config(message: impl Into<String>) -> Self {
        Self::Config {
            message: message.into(),
            source: None,
        }
    }

    /// Creates a new configuration error with source.
    pub fn config_with_source(
        message: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self::Config {
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }

    /// Creates a new database error.
    pub fn database(message: impl Into<String>) -> Self {
        Self::Database {
            message: message.into(),
            source: None,
        }
    }

    /// Creates a new database error with source.
    pub fn database_with_source(
        message: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self::Database {
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }

    /// Creates a new external service error.
    pub fn external_service(service: impl Into<String>, message: impl Into<String>) -> Self {
        Self::ExternalService {
            service: service.into(),
            message: message.into(),
            source: None,
        }
    }

    /// Creates a new external service error with source.
    pub fn external_service_with_source(
        service: impl Into<String>,
        message: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self::ExternalService {
            service: service.into(),
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }

    /// Creates a new authentication error.
    pub fn auth(message: impl Into<String>) -> Self {
        Self::Auth {
            message: message.into(),
            source: None,
        }
    }

    /// Creates a new authentication error with source.
    pub fn auth_with_source(
        message: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self::Auth {
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }

    /// Creates a new file system error.
    pub fn file_system(message: impl Into<String>) -> Self {
        Self::FileSystem {
            message: message.into(),
            source: None,
        }
    }

    /// Creates a new file system error with source.
    pub fn file_system_with_source(
        message: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self::FileSystem {
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }

    /// Creates a new network error.
    pub fn network(message: impl Into<String>) -> Self {
        Self::Network {
            message: message.into(),
            source: None,
        }
    }

    /// Creates a new network error with source.
    pub fn network_with_source(
        message: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self::Network {
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }

    /// Creates a new internal error.
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal {
            message: message.into(),
            source: None,
        }
    }

    /// Creates a new internal error with source.
    pub fn internal_with_source(
        message: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self::Internal {
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }

    /// Returns the error category.
    pub fn category(&self) -> &'static str {
        match self {
            Self::Config { .. } => "configuration",
            Self::Database { .. } => "database",
            Self::ExternalService { .. } => "external_service",
            Self::Auth { .. } => "authentication",
            Self::FileSystem { .. } => "file_system",
            Self::Network { .. } => "network",
            Self::Internal { .. } => "internal",
        }
    }

    /// Converts this service error into a handler error.
    ///
    /// This is used when service errors need to be returned from HTTP handlers.
    pub fn into_handler_error(self) -> crate::handler::Error {
        use crate::handler::ErrorKind;

        match self {
            Self::Config { message, .. } => ErrorKind::InternalServerError.with_context(message),
            Self::Database { message, .. } => ErrorKind::InternalServerError.with_context(message),
            Self::ExternalService { message, .. } => {
                ErrorKind::InternalServerError.with_context(message)
            }
            Self::Auth { message, .. } => ErrorKind::Unauthorized.with_context(message),
            Self::FileSystem { message, .. } => {
                ErrorKind::InternalServerError.with_context(message)
            }
            Self::Network { message, .. } => ErrorKind::InternalServerError.with_context(message),
            Self::Internal { message, .. } => ErrorKind::InternalServerError.with_context(message),
        }
    }
}

/// Conversion from service error to handler error.
impl From<ServiceError> for crate::handler::Error {
    fn from(error: ServiceError) -> Self {
        error.into_handler_error()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        let error = ServiceError::config("Invalid configuration");
        assert_eq!(error.category(), "configuration");
        assert!(error.to_string().contains("Invalid configuration"));
    }

    #[test]
    fn test_error_with_source() {
        let source = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let error = ServiceError::file_system_with_source("Cannot read config file", source);

        assert_eq!(error.category(), "file_system");
        assert!(error.source().is_some());
    }

    #[test]
    fn test_handler_error_conversion() {
        let service_error = ServiceError::auth("Invalid credentials");
        let handler_error = service_error.into_handler_error();

        // Test that it converts to the expected handler error type
        assert!(handler_error.to_string().contains("Invalid credentials"));
    }

    #[test]
    fn test_external_service_error() {
        let error = ServiceError::external_service("MinIO", "Connection failed");
        assert!(error.to_string().contains("MinIO"));
        assert!(error.to_string().contains("Connection failed"));
    }
}
