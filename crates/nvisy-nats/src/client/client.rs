//! NATS client wrapper and connection management.

use std::time::Duration;

use async_nats::{Client, ConnectOptions, jetstream};
use bytes::Bytes;
use tokio::time::timeout;
use tracing::{debug, info, instrument};

use super::config::{NatsConfig, NatsCredentials};
use crate::{Error, Result, TRACING_TARGET_CLIENT, TRACING_TARGET_CONNECTION};

/// NATS client wrapper with connection management
#[derive(Debug, Clone)]
pub struct NatsClient {
    client: Client,
    jetstream: jetstream::Context,
    config: NatsConfig,
}

impl NatsClient {
    /// Create a new NATS client and connect
    #[instrument(skip(config))]
    pub async fn connect(config: NatsConfig) -> Result<Self> {
        info!("Connecting to NATS servers: {:?}", config.servers);

        let mut connect_opts = ConnectOptions::new()
            .name(&config.name)
            .connection_timeout(config.connect_timeout)
            .ping_interval(config.ping_interval);

        // Set reconnection options
        if let Some(max_reconnects) = config.max_reconnects {
            connect_opts = connect_opts.max_reconnects(max_reconnects);
        }
        let reconnect_delay_ms = config.reconnect_delay.as_millis() as u64;
        connect_opts = connect_opts.reconnect_delay_callback(move |attempts| {
            Duration::from_millis(std::cmp::min(
                reconnect_delay_ms * 2_u64.pow(attempts as u32),
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
        info!(
            target: TRACING_TARGET_CONNECTION,
            server_host = %server_info.host,
            server_version = %server_info.version,
            server_id = %server_info.server_id,
            max_payload = server_info.max_payload,
            "Successfully connected to NATS"
        );

        Ok(Self {
            client,
            jetstream,
            config,
        })
    }

    /// Get the underlying NATS client
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Get the JetStream context
    pub fn jetstream(&self) -> &jetstream::Context {
        &self.jetstream
    }

    /// Get the configuration
    pub fn config(&self) -> &NatsConfig {
        &self.config
    }

    /// Create a new connection helper
    pub fn connection(&self) -> NatsConnection {
        NatsConnection {
            client: self.client.clone(),
            request_timeout: self.config.request_timeout,
        }
    }

    /// Test connectivity with a ping
    #[instrument(skip(self), target = TRACING_TARGET_CLIENT)]
    pub async fn ping(&self) -> Result<Duration> {
        let start = std::time::Instant::now();

        timeout(Duration::from_secs(10), self.client.flush())
            .await
            .map_err(|_| Error::Timeout {
                timeout: Duration::from_secs(10),
            })?
            .map_err(|e| Error::Connection(Box::new(e)))?;

        let ping_time = start.elapsed();
        debug!(
            target: TRACING_TARGET_CLIENT,
            duration_ms = ping_time.as_millis(),
            "NATS ping successful"
        );
        Ok(ping_time)
    }

    /// Get connection statistics
    pub fn stats(&self) -> ConnectionStats {
        let server_info = self.client.server_info();
        ConnectionStats {
            server_name: server_info.server_name.clone(),
            server_version: server_info.version.clone(),
            server_id: server_info.server_id.clone(),
            is_connected: matches!(
                self.client.connection_state(),
                async_nats::connection::State::Connected
            ),
            max_payload: server_info.max_payload,
        }
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
    #[instrument(skip(self, payload))]
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

        debug!(
            target: TRACING_TARGET_CLIENT,
            subject = %subject,
            "Published message"
        );
        Ok(())
    }

    /// Publish a message with a reply subject
    #[instrument(skip(self, payload), target = TRACING_TARGET_CLIENT)]
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

        debug!(
            target: TRACING_TARGET_CLIENT,
            subject = %subject,
            reply = %reply,
            "Published message with reply"
        );
        Ok(())
    }

    /// Send a request and wait for a response
    #[instrument(skip(self, payload), target = TRACING_TARGET_CLIENT)]
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

        debug!(
            target: TRACING_TARGET_CLIENT,
            subject = %subject,
            payload_size = response.payload.len(),
            "Received response for request"
        );
        Ok(response)
    }

    /// Subscribe to a subject
    #[instrument(skip(self), target = TRACING_TARGET_CLIENT)]
    pub async fn subscribe(&self, subject: &str) -> Result<async_nats::Subscriber> {
        let subscriber = self
            .client
            .subscribe(subject.to_string())
            .await
            .map_err(|e| Error::Connection(Box::new(e)))?;

        debug!(
            target: TRACING_TARGET_CLIENT,
            subject = %subject,
            "Subscribed to subject"
        );
        Ok(subscriber)
    }

    /// Subscribe to a subject with a queue group
    #[instrument(skip(self), target = TRACING_TARGET_CLIENT)]
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

        debug!(
            target: TRACING_TARGET_CLIENT,
            subject = %subject,
            queue = %queue,
            "Subscribed to subject with queue group"
        );
        Ok(subscriber)
    }

    /// Flush pending messages
    #[instrument(skip(self), target = TRACING_TARGET_CLIENT)]
    pub async fn flush(&self) -> Result<()> {
        timeout(self.request_timeout, self.client.flush())
            .await
            .map_err(|_| Error::Timeout {
                timeout: self.request_timeout,
            })?
            .map_err(|e| Error::Connection(Box::new(e)))?;

        debug!(
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
