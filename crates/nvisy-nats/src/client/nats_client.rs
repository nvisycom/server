//! NATS client wrapper and connection management.
//!
//! # Connection Pooling and Multiplexing
//!
//! The `NatsClient` uses the underlying `async-nats` client which implements
//! connection multiplexing. Key characteristics:
//!
//! - **Single TCP connection**: Each `Client` maintains one TCP connection to NATS
//! - **Thread-safe and Clone-able**: The `Client` is `Arc`-wrapped internally,
//!   making `clone()` operations cheap (just an Arc clone, not a new connection)
//! - **Concurrent operations**: Multiple async tasks can share the same client
//!   and perform operations concurrently over the same connection
//! - **Automatic reconnection**: Built-in reconnection logic with exponential backoff
//!
//! ## Usage Patterns
//!
//! ### Single shared client (recommended)
//! ```ignore
//! let client = NatsClient::connect(config).await?;
//! // Clone is cheap - shares the same connection
//! let client_clone = client.clone();
//! ```
//!
//! ### Connection per service (if needed)
//! Only create multiple connections if you need different configurations
//! (credentials, timeouts, etc.) or want to isolate failure domains:
//! ```ignore
//! let auth_client = NatsClient::connect(auth_config).await?;
//! let data_client = NatsClient::connect(data_config).await?;
//! ```

use std::sync::Arc;
use std::time::Duration;

use async_nats::{Client, ConnectOptions, jetstream};
use bytes::Bytes;
use tokio::time::timeout;

use super::nats_config::{NatsConfig, NatsCredentials};
use crate::kv::{ApiTokenStore, CacheStore, KvStore};
use crate::object::{DocumentFileStore, DocumentLabel, ObjectStore};
use crate::{Error, Result, TRACING_TARGET_CLIENT, TRACING_TARGET_CONNECTION};

/// NATS client wrapper with connection management.
///
/// This wrapper is cheaply cloneable and thread-safe.
/// Multiple clones share the same underlying TCP connection via multiplexing.
#[derive(Debug, Clone)]
pub struct NatsClient {
    inner: Arc<NatsClientInner>,
}

/// Inner data for NATS client
#[derive(Debug)]
struct NatsClientInner {
    client: Client,
    jetstream: jetstream::Context,
    config: NatsConfig,
}

impl NatsClient {
    /// Create a new NATS client and connect
    #[tracing::instrument(skip(config))]
    pub async fn connect(config: NatsConfig) -> Result<Self> {
        tracing::info!("connecting to NATS servers: {:?}", config.servers);

        let mut connect_opts = ConnectOptions::new()
            .name(&config.name)
            .connection_timeout(config.connect_timeout)
            .ping_interval(config.ping_interval);

        // Set reconnection options
        if let Some(max_reconnects) = config.max_reconnects {
            connect_opts = connect_opts.max_reconnects(max_reconnects);
        }
        let reconnect_delay_ms = config.reconnect_delay.as_millis().min(u64::MAX as u128) as u64;
        connect_opts = connect_opts.reconnect_delay_callback(move |attempts| {
            Duration::from_millis(std::cmp::min(
                reconnect_delay_ms * 2_u64.pow(attempts.min(32) as u32),
                30_000, // Max 30 seconds
            ))
        });

        // Set authentication if provided
        if let Some(credentials) = &config.credentials {
            connect_opts = match credentials {
                NatsCredentials::UserPassword { user, pass } => {
                    connect_opts.user_and_password(user.clone(), pass.clone())
                }
                NatsCredentials::Token { token } => connect_opts.token(token.clone()),
                NatsCredentials::CredsFile { path } => connect_opts
                    .credentials_file(path)
                    .await
                    .map_err(|e| Error::operation("credentials_file", e.to_string()))?,
                NatsCredentials::NKey { seed } => connect_opts.nkey(seed.clone()),
            };
        }

        // Set TLS if configured
        if let Some(tls_config) = &config.tls
            && tls_config.enabled
        {
            connect_opts = connect_opts.require_tls(true);
            // Note: Custom TLS verification requires using rustls directly
            // For production, use proper certificate validation
        }

        // Connect to NATS
        let client = timeout(
            config.connect_timeout,
            async_nats::connect_with_options(&config.servers.join(","), connect_opts),
        )
        .await
        .map_err(|_| Error::Timeout {
            timeout: config.connect_timeout,
        })?
        .map_err(|e| Error::Connection(Box::new(e)))?;

        // Initialize JetStream context
        let jetstream = jetstream::new(client.clone());

        let server_info = client.server_info();
        tracing::info!(
            target: TRACING_TARGET_CONNECTION,
            server_host = %server_info.host,
            server_version = %server_info.version,
            server_id = %server_info.server_id,
            max_payload = server_info.max_payload,
            "Successfully connected to NATS"
        );

        Ok(Self {
            inner: Arc::new(NatsClientInner {
                client,
                jetstream,
                config,
            }),
        })
    }

    /// Get the underlying NATS client
    #[must_use]
    pub fn client(&self) -> &Client {
        &self.inner.client
    }

    /// Get the JetStream context
    #[must_use]
    pub fn jetstream(&self) -> &jetstream::Context {
        &self.inner.jetstream
    }

    /// Get the configuration
    #[must_use]
    pub fn config(&self) -> &NatsConfig {
        &self.inner.config
    }

    /// Test connectivity with a ping
    #[tracing::instrument(skip(self), target = TRACING_TARGET_CONNECTION)]
    pub async fn ping(&self) -> Result<Duration> {
        let start = std::time::Instant::now();

        timeout(Duration::from_secs(10), self.inner.client.flush())
            .await
            .map_err(|_| Error::Timeout {
                timeout: Duration::from_secs(10),
            })?
            .map_err(|e| Error::Connection(Box::new(e)))?;

        let ping_time = start.elapsed();
        tracing::debug!(
            target: TRACING_TARGET_CLIENT,
            duration_ms = ping_time.as_millis(),
            "NATS ping successful"
        );
        Ok(ping_time)
    }

    /// Get connection statistics
    #[must_use]
    pub fn stats(&self) -> ConnectionStats {
        let server_info = self.inner.client.server_info();
        ConnectionStats {
            server_name: server_info.server_name.clone(),
            server_version: server_info.version.clone(),
            server_id: server_info.server_id.clone(),
            is_connected: matches!(
                self.inner.client.connection_state(),
                async_nats::connection::State::Connected
            ),
            max_payload: server_info.max_payload,
        }
    }

    /// Get or create an ApiTokenStore
    #[tracing::instrument(skip(self), target = TRACING_TARGET_CLIENT)]
    pub async fn api_token_store(&self, ttl: Option<Duration>) -> Result<ApiTokenStore> {
        ApiTokenStore::new(&self.inner.jetstream, ttl).await
    }

    /// Get or create a KvStore for a specific bucket with typed values
    #[tracing::instrument(skip(self), target = TRACING_TARGET_CLIENT)]
    pub async fn kv_store<T>(
        &self,
        bucket_name: &str,
        description: Option<&str>,
        ttl: Option<Duration>,
    ) -> Result<KvStore<T>>
    where
        T: serde::Serialize + for<'de> serde::Deserialize<'de> + Send + Sync + 'static,
    {
        KvStore::new(&self.inner.jetstream, bucket_name, description, ttl).await
    }

    /// Get or create a typed ObjectStore for a specific bucket with custom key and data types
    #[tracing::instrument(skip(self), target = TRACING_TARGET_CLIENT)]
    pub async fn object_store<K: AsRef<str>>(
        &self,
        bucket_name: &str,
        description: Option<&str>,
        max_age: Option<Duration>,
    ) -> Result<ObjectStore<K>> {
        ObjectStore::new(&self.inner.jetstream, bucket_name, description, max_age).await
    }

    /// Get or create a typed ObjectStore for a specific document label.
    #[tracing::instrument(skip(self), target = TRACING_TARGET_CLIENT)]
    pub async fn document_store<S: DocumentLabel>(&self) -> Result<DocumentFileStore<S>> {
        DocumentFileStore::new(&self.inner.jetstream).await
    }

    /// Get or create a CacheStore for a specific namespace
    #[tracing::instrument(skip(self), target = TRACING_TARGET_CLIENT)]
    pub async fn cache_store<T>(
        &self,
        namespace: &str,
        ttl: Option<Duration>,
    ) -> Result<CacheStore<T>>
    where
        T: serde::Serialize + for<'de> serde::Deserialize<'de> + Clone + Send + Sync + 'static,
    {
        CacheStore::new(&self.inner.jetstream, namespace, ttl).await
    }
}

/// A NATS connection wrapper for basic pub/sub operations
#[derive(Debug, Clone)]
pub struct NatsConnection {
    client: Client,
    request_timeout: Duration,
}

impl NatsConnection {
    /// Publish a message to a subject
    #[tracing::instrument(skip(self, payload))]
    pub async fn publish(&self, subject: &str, payload: impl Into<Bytes>) -> Result<()> {
        timeout(
            self.request_timeout,
            self.client.publish(subject.to_string(), payload.into()),
        )
        .await
        .map_err(|_| Error::Timeout {
            timeout: self.request_timeout,
        })?
        .map_err(|e| Error::delivery_failed(subject, e.to_string()))?;

        tracing::debug!(
            target: TRACING_TARGET_CLIENT,
            subject = %subject,
            "Published message"
        );
        Ok(())
    }

    /// Publish a message with a reply subject
    #[tracing::instrument(skip(self, payload), target = TRACING_TARGET_CLIENT)]
    pub async fn publish_with_reply(
        &self,
        subject: &str,
        reply: &str,
        payload: impl Into<Bytes>,
    ) -> Result<()> {
        timeout(
            self.request_timeout,
            self.client
                .publish_with_reply(subject.to_string(), reply.to_string(), payload.into()),
        )
        .await
        .map_err(|_| Error::Timeout {
            timeout: self.request_timeout,
        })?
        .map_err(|e| Error::delivery_failed(subject, e.to_string()))?;

        tracing::debug!(
            target: TRACING_TARGET_CLIENT,
            subject = %subject,
            reply = %reply,
            "Published message with reply"
        );
        Ok(())
    }

    /// Send a request and wait for a response
    #[tracing::instrument(skip(self, payload), target = TRACING_TARGET_CLIENT)]
    pub async fn request(
        &self,
        subject: &str,
        payload: impl Into<Bytes>,
    ) -> Result<async_nats::Message> {
        let response = timeout(
            self.request_timeout,
            self.client.request(subject.to_string(), payload.into()),
        )
        .await
        .map_err(|_| Error::Timeout {
            timeout: self.request_timeout,
        })?
        .map_err(|e| Error::delivery_failed(subject, e.to_string()))?;

        tracing::debug!(
            target: TRACING_TARGET_CLIENT,
            subject = %subject,
            payload_size = response.payload.len(),
            "Received response for request"
        );
        Ok(response)
    }

    /// Subscribe to a subject
    #[tracing::instrument(skip(self), target = TRACING_TARGET_CLIENT)]
    pub async fn subscribe(&self, subject: &str) -> Result<async_nats::Subscriber> {
        let subscriber = self
            .client
            .subscribe(subject.to_string())
            .await
            .map_err(|e| Error::Connection(Box::new(e)))?;

        tracing::debug!(
            target: TRACING_TARGET_CLIENT,
            subject = %subject,
            "Subscribed to subject"
        );
        Ok(subscriber)
    }

    /// Subscribe to a subject with a queue group
    #[tracing::instrument(skip(self), target = TRACING_TARGET_CLIENT)]
    pub async fn queue_subscribe(
        &self,
        subject: &str,
        queue: &str,
    ) -> Result<async_nats::Subscriber> {
        let subscriber = self
            .client
            .queue_subscribe(subject.to_string(), queue.to_string())
            .await
            .map_err(|e| Error::Connection(Box::new(e)))?;

        tracing::debug!(
            target: TRACING_TARGET_CLIENT,
            subject = %subject,
            queue = %queue,
            "Subscribed to subject with queue group"
        );
        Ok(subscriber)
    }

    /// Flush pending messages
    #[tracing::instrument(skip(self), target = TRACING_TARGET_CLIENT)]
    pub async fn flush(&self) -> Result<()> {
        timeout(self.request_timeout, self.client.flush())
            .await
            .map_err(|_| Error::Timeout {
                timeout: self.request_timeout,
            })?
            .map_err(|e| Error::Connection(Box::new(e)))?;

        tracing::debug!(
            target: TRACING_TARGET_CLIENT,
            "Flushed pending messages"
        );
        Ok(())
    }
}

/// Connection statistics
#[derive(Debug, Clone)]
pub struct ConnectionStats {
    pub server_name: String,
    pub server_version: String,
    pub server_id: String,
    pub is_connected: bool,
    pub max_payload: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_stats() {
        let stats = ConnectionStats {
            server_name: "test-server".to_string(),
            server_version: "2.9.0".to_string(),
            server_id: "server123".to_string(),
            is_connected: true,
            max_payload: 1048576,
        };

        assert_eq!(stats.server_name, "test-server");
        assert!(stats.is_connected);
        assert_eq!(stats.max_payload, 1048576);
    }
}
