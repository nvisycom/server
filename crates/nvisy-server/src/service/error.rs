//! Service layer error types.
//!
//! This module provides comprehensive error handling for the service layer,
//! covering configuration, database, external service, and system errors.

use std::error::Error;

/// Result type for service operations.
pub type Result<T> = std::result::Result<T, ServiceError>;

/// Service layer error types.
///
/// These errors represent failures in the service layer, such as configuration
/// issues, database connectivity problems, external service failures, etc.
#[derive(Debug, thiserror::Error)]
pub enum ServiceError {
    /// Configuration error (invalid config values, missing files, etc.).
    #[error("Configuration error: {message}")]
    Config {
        message: String,
        #[source]
        source: Option<Box<dyn Error + Send + Sync>>,
    },

    /// Database connection or query error.
    #[error("Database error: {message}")]
    Database {
        message: String,
        #[source]
        source: Option<Box<dyn Error + Send + Sync>>,
    },

    /// External service error (NATS, OpenRouter, etc.).
    #[error("External service error ({service}): {message}")]
    ExternalService {
        service: String,
        message: String,
        #[source]
        source: Option<Box<dyn Error + Send + Sync>>,
    },

    /// Authentication or authorization error.
    #[error("Authentication error: {message}")]
    Auth {
        message: String,
        #[source]
        source: Option<Box<dyn Error + Send + Sync>>,
    },

    /// File system operation error.
    #[error("File system error: {message}")]
    FileSystem {
        message: String,
        #[source]
        source: Option<Box<dyn Error + Send + Sync>>,
    },

    /// Internal service error.
    #[error("Internal service error: {message}")]
    InternalService {
        message: String,
        #[source]
        source: Option<Box<dyn Error + Send + Sync>>,
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
        source: impl Error + Send + Sync + 'static,
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
        source: impl Error + Send + Sync + 'static,
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
        source: impl Error + Send + Sync + 'static,
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
        source: impl Error + Send + Sync + 'static,
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
        source: impl Error + Send + Sync + 'static,
    ) -> Self {
        Self::FileSystem {
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }

    /// Creates a new internal error.
    pub fn internal(message: impl Into<String>) -> Self {
        Self::InternalService {
            message: message.into(),
            source: None,
        }
    }

    /// Creates a new internal error with source.
    pub fn internal_with_source(
        message: impl Into<String>,
        source: impl Error + Send + Sync + 'static,
    ) -> Self {
        Self::InternalService {
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        let error = ServiceError::config("Invalid configuration");
        assert!(error.to_string().contains("Invalid configuration"));
    }

    #[test]
    fn test_error_with_source() {
        let source = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let error = ServiceError::file_system_with_source("Cannot read config file", source);
        assert!(std::error::Error::source(&error).is_some());
    }
}
