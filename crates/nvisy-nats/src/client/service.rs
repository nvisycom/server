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
use std::time::{Duration, Instant};

use async_nats::connection::State;
use async_nats::{Client, ConnectOptions, jetstream};
use nvisy_core::health::{ComponentHealth, HealthCheck};
use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::time::timeout;

use super::config::NatsConfig;
use crate::kv::{KvBucket, KvStore};
use crate::stream::core::ensure_stream;
use crate::stream::{BroadcastStream, EventPublisher, EventStream, EventSubscriber};
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
        tracing::info!("Connecting to NATS servers: {}", config.nats_url);

        let mut connect_opts = ConnectOptions::new()
            .name(config.name())
            .ping_interval(config.ping_interval())
            .token(config.nats_token.clone());

        // Set connection timeout if specified
        if let Some(timeout) = config.nats_connect_timeout {
            connect_opts = connect_opts.connection_timeout(timeout);
        }

        // Set the request-reply timeout if specified (overrides async-nats' 10s default).
        if let Some(timeout) = config.nats_request_timeout {
            connect_opts = connect_opts.request_timeout(Some(timeout));
        }

        // Set reconnection options
        if let Some(max_reconnects) = config.max_reconnects_option() {
            connect_opts = connect_opts.max_reconnects(max_reconnects);
        }
        let reconnect_delay_ms =
            u64::try_from(config.reconnect_delay().as_millis()).unwrap_or(u64::MAX);
        connect_opts = connect_opts.reconnect_delay_callback(move |attempts| {
            let exponent = u32::try_from(attempts.min(32)).unwrap_or(32);
            Duration::from_millis(std::cmp::min(
                reconnect_delay_ms * 2_u64.pow(exponent),
                30_000, // Max 30 seconds
            ))
        });

        // Connect to NATS
        // Use configured timeout or a sensible default (30 seconds)
        let connect_timeout = config
            .nats_connect_timeout
            .unwrap_or(Duration::from_secs(30));
        let client = timeout(
            connect_timeout,
            async_nats::connect_with_options(&config.nats_url, connect_opts),
        )
        .await
        .map_err(|_| Error::Timeout {
            timeout: connect_timeout,
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

    /// Get the configuration
    #[must_use]
    pub fn config(&self) -> &NatsConfig {
        &self.inner.config
    }

    /// Test connectivity with a ping
    #[tracing::instrument(skip(self), target = TRACING_TARGET_CONNECTION)]
    pub async fn ping(&self) -> Result<Duration> {
        let start = Instant::now();

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

    /// Check if the client is connected.
    #[must_use]
    pub fn is_connected(&self) -> bool {
        matches!(self.inner.client.connection_state(), State::Connected)
    }
}

// Key-value store getters
impl NatsClient {
    /// Get or create the KV store for a bucket. The bucket fixes the key and
    /// value types, so it is the only type argument.
    #[tracing::instrument(skip(self), target = TRACING_TARGET_CLIENT)]
    pub async fn kv_store<B: KvBucket>(&self) -> Result<KvStore<B>> {
        KvStore::new(&self.inner.jetstream).await
    }

    /// Get or create the KV store for a bucket with a custom TTL.
    #[tracing::instrument(skip(self), target = TRACING_TARGET_CLIENT)]
    pub async fn kv_store_with_ttl<B: KvBucket>(&self, ttl: Duration) -> Result<KvStore<B>> {
        KvStore::with_ttl(&self.inner.jetstream, ttl).await
    }
}

// Stream getters
impl NatsClient {
    /// Ensures the `JetStream` stream backing `S` exists and matches `S`'s config,
    /// creating it if absent and reconciling subjects/retention if present.
    ///
    /// This is a per-deployment concern: call it once at startup for each stream,
    /// so that publishers and subscribers built later are cheap handles that do no
    /// reconciliation of their own.
    #[tracing::instrument(skip(self), target = TRACING_TARGET_CLIENT)]
    pub async fn ensure_stream<S: EventStream>(&self) -> Result<()> {
        ensure_stream::<S>(&self.inner.jetstream).await
    }

    /// Create an event publisher for the specified stream type.
    ///
    /// A cheap handle over the `JetStream` context; the stream must already have
    /// been reconciled at startup via [`ensure_stream`](Self::ensure_stream).
    #[must_use]
    pub fn event_publisher<S: EventStream>(&self) -> EventPublisher<S> {
        EventPublisher::new(&self.inner.jetstream)
    }

    /// Create an event subscriber for the specified stream type.
    ///
    /// A cheap handle over the `JetStream` context; the stream must already have
    /// been reconciled at startup via [`ensure_stream`](Self::ensure_stream). The
    /// durable consumer is upserted when [`subscribe`](EventSubscriber::subscribe)
    /// is called.
    #[must_use]
    pub fn event_subscriber<S: EventStream>(&self) -> EventSubscriber<S> {
        EventSubscriber::new(&self.inner.jetstream)
    }
}

// Core NATS pub/sub (non-JetStream broadcast).
//
// Unlike the JetStream helpers above, these use plain NATS subjects: fire-and-
// forget, no persistence, and every subscriber to a subject receives every
// message (fan-out). Suited to ephemeral fan-out where the durable source of
// truth lives elsewhere (e.g. streaming run-status changes to any number of
// watching SSE connections; the run row in Postgres is authoritative).
impl NatsClient {
    /// Publishes a JSON-serialized message to a core NATS subject.
    ///
    /// Best-effort: delivered to whichever subscribers are connected now, with no
    /// persistence or acknowledgement.
    #[tracing::instrument(skip(self, message), target = TRACING_TARGET_CLIENT)]
    pub async fn publish_broadcast<T>(&self, subject: String, message: &T) -> Result<()>
    where
        T: Serialize,
    {
        let payload = serde_json::to_vec(message)?;
        self.inner
            .client
            .publish(subject, payload.into())
            .await
            .map_err(|e| Error::Connection(Box::new(e)))?;
        Ok(())
    }

    /// Subscribes to a core NATS subject, yielding each deserialized message.
    ///
    /// The stream ends when the subscription is dropped. Messages that fail to
    /// deserialize are skipped rather than ending the stream.
    #[tracing::instrument(skip(self), target = TRACING_TARGET_CLIENT)]
    pub async fn subscribe_broadcast<T>(&self, subject: String) -> Result<BroadcastStream<T>>
    where
        T: DeserializeOwned + Send + 'static,
    {
        let subscriber = self
            .inner
            .client
            .subscribe(subject)
            .await
            .map_err(|e| Error::Connection(Box::new(e)))?;

        Ok(BroadcastStream::new(subscriber))
    }
}

/// Component name reported for the NATS health check.
const HEALTH_COMPONENT_NAME: &str = "nats";

#[async_trait::async_trait]
impl HealthCheck for NatsClient {
    /// Probes NATS by checking the connection state, then pinging the server.
    async fn check_health(&self) -> ComponentHealth {
        if !self.is_connected() {
            tracing::warn!(target: TRACING_TARGET_CONNECTION, "NATS is not connected");
            return ComponentHealth::unhealthy(HEALTH_COMPONENT_NAME);
        }

        match self.ping().await {
            Ok(latency) => {
                tracing::debug!(
                    target: TRACING_TARGET_CONNECTION,
                    ping_ms = latency.as_millis(),
                    "NATS health check passed"
                );
                ComponentHealth::healthy(HEALTH_COMPONENT_NAME)
            }
            Err(e) => {
                tracing::warn!(
                    target: TRACING_TARGET_CONNECTION,
                    error = %e,
                    "NATS health check failed"
                );
                ComponentHealth::unhealthy(HEALTH_COMPONENT_NAME)
            }
        }
    }
}
