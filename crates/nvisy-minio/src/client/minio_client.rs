//! High-level MinIO client implementation.
//!
//! This module provides the main client interface for MinIO operations,
//! encapsulating connection management, configuration, and error handling.

use std::sync::Arc;

use minio::s3::Client;
use minio::s3::creds::StaticProvider;
use minio::s3::types::S3Api;
use tracing::{debug, error, info, instrument};

use crate::operations::{BucketOperations, ObjectOperations};
use crate::{Error, MinioConfig, Result, TRACING_TARGET_CLIENT, TRACING_TARGET_OPERATIONS};

/// High-level MinIO client that manages connections and operations.
///
/// This struct provides the main interface for MinIO operations, encapsulating
/// client configuration, connection management, and error handling.
#[derive(Clone)]
pub struct MinioClient {
    inner: Client,
    config: Arc<MinioConfig>,
}

impl MinioClient {
    /// Creates a new MinIO client with the provided configuration.
    ///
    /// This will create a MinIO client instance with the specified configuration
    /// but does not test connectivity.
    ///
    /// # Arguments
    ///
    /// * `config` - MinIO configuration including endpoint, credentials, and other settings
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Configuration validation fails
    /// - Client initialization fails
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nvisy_minio::{MinioClient, MinioConfig, MinioCredentials};
    /// use url::Url;
    ///
    /// let endpoint = Url::parse("https://play.min.io").unwrap();
    /// let credentials = MinioCredentials::new("access_key", "secret_key");
    /// let config = MinioConfig::new(endpoint, credentials).unwrap();
    /// let client = MinioClient::new(config).unwrap();
    /// ```
    #[instrument(skip(config), target = TRACING_TARGET_CLIENT, fields(endpoint = %config.endpoint_masked()))]
    pub fn new(config: MinioConfig) -> Result<Self> {
        info!(target: TRACING_TARGET_CLIENT, "Initializing MinIO client");

        // Validate configuration first
        config.validate().map_err(|e| {
            error!(target: TRACING_TARGET_CLIENT, error = %e, "Configuration validation failed");
            e
        })?;

        // Create credentials provider
        let provider = StaticProvider::from(config.credentials().clone());

        // Create MinIO client with HTTPS endpoint (enforced by config validation)
        let endpoint_url = config.endpoint().to_string();

        let endpoint = endpoint_url.parse().map_err(|e| {
            error!(target: TRACING_TARGET_CLIENT, error = %e, "Invalid endpoint URL");
            Error::Config(format!("Invalid endpoint URL: {}", e))
        })?;

        let provider = Box::new(provider);
        let inner = Client::new(endpoint, Some(provider), None, None).map_err(|e| {
            error!(target: TRACING_TARGET_CLIENT, error = %e, "Failed to create MinIO client");
            Error::Config(format!("Failed to build MinIO client: {}", e))
        })?;

        info!(
            target: TRACING_TARGET_CLIENT,
            endpoint = %config.endpoint_masked(),
            secure = config.is_secure(),
            path_style = config.path_style,
            "MinIO client initialized successfully"
        );

        Ok(Self {
            inner,
            config: Arc::new(config),
        })
    }

    /// Creates a new MinIO client with the provided configuration and tests connectivity.
    ///
    /// This will create a MinIO client instance and verify that it can connect
    /// to the MinIO server by attempting to list buckets.
    ///
    /// # Arguments
    ///
    /// * `config` - MinIO configuration including endpoint, credentials, and other settings
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Configuration validation fails
    /// - Client initialization fails
    /// - Connectivity test fails
    #[instrument(skip(config), target = TRACING_TARGET_CLIENT, fields(endpoint = %config.endpoint_masked()))]
    pub async fn new_with_test(config: MinioConfig) -> Result<Self> {
        let client = Self::new(config)?;

        // Test connectivity by attempting to list buckets
        debug!(target: TRACING_TARGET_OPERATIONS, "Testing MinIO connectivity");

        let start = std::time::Instant::now();
        client.inner.list_buckets().send().await.map_err(|e| {
            error!(
                target: TRACING_TARGET_OPERATIONS,
                error = %e,
                elapsed = ?start.elapsed(),
                "MinIO connectivity test failed"
            );
            Error::Client(e)
        })?;

        let elapsed = start.elapsed();
        info!(
            target: TRACING_TARGET_CLIENT,
            elapsed = ?elapsed,
            "MinIO connectivity test successful"
        );

        Ok(client)
    }

    /// Tests the connection to the MinIO server.
    ///
    /// This method attempts to list buckets to verify that the client can
    /// successfully communicate with the MinIO server.
    ///
    /// # Errors
    ///
    /// Returns an error if the connection test fails due to:
    /// - Network connectivity issues
    /// - Authentication failures
    /// - Server unavailability
    #[instrument(skip(self), target = TRACING_TARGET_OPERATIONS)]
    pub async fn test_connection(&self) -> Result<()> {
        debug!(target: TRACING_TARGET_OPERATIONS, "Testing MinIO connection");

        let start = std::time::Instant::now();
        let result = self.inner.list_buckets().send().await;
        let elapsed = start.elapsed();

        match result {
            Ok(_) => {
                debug!(
                    target: TRACING_TARGET_OPERATIONS,
                    elapsed = ?elapsed,
                    "Connection test successful"
                );
                Ok(())
            }
            Err(e) => {
                error!(
                    target: TRACING_TARGET_OPERATIONS,
                    error = %e,
                    elapsed = ?elapsed,
                    "Connection test failed"
                );
                Err(Error::Client(e))
            }
        }
    }

    /// Performs a health check on the MinIO connection.
    ///
    /// This is a lightweight operation that verifies the client can
    /// communicate with the MinIO server. It's suitable for use in
    /// health check endpoints and monitoring systems.
    ///
    /// # Errors
    ///
    /// Returns an error if the health check fails.
    #[instrument(skip(self), target = TRACING_TARGET_OPERATIONS)]
    pub async fn health_check(&self) -> Result<()> {
        self.test_connection().await
    }

    /// Creates a new BucketOperations instance.
    pub fn bucket_operations(&self) -> BucketOperations {
        BucketOperations::new(self.clone())
    }

    /// Creates a new ObjectOperations instance.
    pub fn object_operations(&self) -> ObjectOperations {
        ObjectOperations::new(self.clone())
    }

    /// Returns a reference to the inner client.
    #[inline]
    pub(crate) fn as_inner(&self) -> &Client {
        &self.inner
    }
}

impl std::fmt::Debug for MinioClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MinioClient")
            .field("endpoint", &self.config.endpoint_masked())
            .field("secure", &self.config.is_secure())
            .field("path_style", &self.config.path_style)
            .field("connect_timeout", &self.config.connect_timeout)
            .field("request_timeout", &self.config.request_timeout)
            .field("access_key", &self.config.credentials().access_key_masked())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use url::Url;

    use super::*;
    use crate::MinioCredentials;

    fn create_test_config() -> MinioConfig {
        let endpoint = Url::parse("https://localhost:9000").unwrap();
        let credentials = MinioCredentials::new("minioadmin", "minioadmin");
        MinioConfig::new(endpoint, credentials).unwrap()
    }

    #[test]
    fn test_client_creation() {
        let config = create_test_config();
        let client = MinioClient::new(config);
        assert!(client.is_ok());
    }

    #[test]
    fn test_client_invalid_config() {
        let endpoint = Url::parse("https://localhost:9000").unwrap();
        let credentials = MinioCredentials::new("", ""); // Invalid empty credentials
        let config = MinioConfig::new(endpoint, credentials).unwrap();

        let client = MinioClient::new(config);
        assert!(client.is_err());
    }

    #[test]
    fn test_client_debug() {
        let config = create_test_config();
        let client = MinioClient::new(config).unwrap();
        let debug_str = format!("{:?}", client);

        assert!(debug_str.contains("MinioClient"));
        assert!(debug_str.contains("localhost:9000"));
        assert!(!debug_str.contains("minioadmin")); // Should be masked
    }
}
