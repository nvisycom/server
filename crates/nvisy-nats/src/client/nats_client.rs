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
use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::time::timeout;

use super::nats_config::NatsConfig;
use crate::kv::{
    ApiToken, ApiTokensBucket, ChatHistoryBucket, KvBucket, KvKey, KvStore, SessionKey, TokenKey,
};
use crate::object::{
    AccountKey, AvatarsBucket, FileKey, FilesBucket, IntermediatesBucket, ObjectBucket, ObjectKey,
    ObjectStore, ThumbnailsBucket,
};
use crate::stream::{EventPublisher, EventStream, EventSubscriber, FileStream, WebhookStream};
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
        if let Some(timeout) = config.connect_timeout() {
            connect_opts = connect_opts.connection_timeout(timeout);
        }

        // Set reconnection options
        if let Some(max_reconnects) = config.max_reconnects_option() {
            connect_opts = connect_opts.max_reconnects(max_reconnects);
        }
        let reconnect_delay_ms = config.reconnect_delay().as_millis().min(u64::MAX as u128) as u64;
        connect_opts = connect_opts.reconnect_delay_callback(move |attempts| {
            Duration::from_millis(std::cmp::min(
                reconnect_delay_ms * 2_u64.pow(attempts.min(32) as u32),
                30_000, // Max 30 seconds
            ))
        });

        // Connect to NATS
        // Use configured timeout or a sensible default (30 seconds)
        let connect_timeout = config.connect_timeout().unwrap_or(Duration::from_secs(30));
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

    /// Check if the client is connected.
    #[must_use]
    pub fn is_connected(&self) -> bool {
        matches!(
            self.inner.client.connection_state(),
            async_nats::connection::State::Connected
        )
    }
}

// Key-value store getters
impl NatsClient {
    /// Get or create a KV store for the specified key, value, and bucket types.
    #[tracing::instrument(skip(self), target = TRACING_TARGET_CLIENT)]
    pub async fn kv_store<K, V, B>(&self) -> Result<KvStore<K, V, B>>
    where
        K: KvKey,
        V: Serialize + DeserializeOwned + Send + Sync + 'static,
        B: KvBucket,
    {
        KvStore::new(&self.inner.jetstream).await
    }

    /// Get or create a KV store with custom TTL.
    #[tracing::instrument(skip(self), target = TRACING_TARGET_CLIENT)]
    pub async fn kv_store_with_ttl<K, V, B>(&self, ttl: Duration) -> Result<KvStore<K, V, B>>
    where
        K: KvKey,
        V: Serialize + DeserializeOwned + Send + Sync + 'static,
        B: KvBucket,
    {
        KvStore::with_ttl(&self.inner.jetstream, ttl).await
    }

    /// Get or create an API token store.
    #[tracing::instrument(skip(self), target = TRACING_TARGET_CLIENT)]
    pub async fn api_token_store(
        &self,
        ttl: Duration,
    ) -> Result<KvStore<TokenKey, ApiToken, ApiTokensBucket>> {
        self.kv_store_with_ttl(ttl).await
    }

    /// Get or create a chat history store with default TTL.
    #[tracing::instrument(skip(self), target = TRACING_TARGET_CLIENT)]
    pub async fn chat_history_store<V>(&self) -> Result<KvStore<SessionKey, V, ChatHistoryBucket>>
    where
        V: Serialize + DeserializeOwned + Send + Sync + 'static,
    {
        self.kv_store().await
    }

    /// Get or create a chat history store with custom TTL.
    #[tracing::instrument(skip(self), target = TRACING_TARGET_CLIENT)]
    pub async fn chat_history_store_with_ttl<V>(
        &self,
        ttl: Duration,
    ) -> Result<KvStore<SessionKey, V, ChatHistoryBucket>>
    where
        V: Serialize + DeserializeOwned + Send + Sync + 'static,
    {
        self.kv_store_with_ttl(ttl).await
    }
}

// Object store getters
impl NatsClient {
    /// Get or create an object store for the specified bucket and key types.
    #[tracing::instrument(skip(self), target = TRACING_TARGET_CLIENT)]
    pub async fn object_store<B, K>(&self) -> Result<ObjectStore<B, K>>
    where
        B: ObjectBucket,
        K: ObjectKey,
    {
        ObjectStore::new(&self.inner.jetstream).await
    }

    /// Get or create a file store for primary file storage.
    #[tracing::instrument(skip(self), target = TRACING_TARGET_CLIENT)]
    pub async fn file_store(&self) -> Result<ObjectStore<FilesBucket, FileKey>> {
        self.object_store().await
    }

    /// Get or create an intermediates store for temporary processing artifacts.
    #[tracing::instrument(skip(self), target = TRACING_TARGET_CLIENT)]
    pub async fn intermediates_store(&self) -> Result<ObjectStore<IntermediatesBucket, FileKey>> {
        self.object_store().await
    }

    /// Get or create a thumbnail store for document thumbnails.
    #[tracing::instrument(skip(self), target = TRACING_TARGET_CLIENT)]
    pub async fn thumbnail_store(&self) -> Result<ObjectStore<ThumbnailsBucket, FileKey>> {
        self.object_store().await
    }

    /// Get or create an avatar store for account avatars.
    #[tracing::instrument(skip(self), target = TRACING_TARGET_CLIENT)]
    pub async fn avatar_store(&self) -> Result<ObjectStore<AvatarsBucket, AccountKey>> {
        self.object_store().await
    }
}

// Stream getters
impl NatsClient {
    /// Create an event publisher for the specified stream type.
    #[tracing::instrument(skip(self), target = TRACING_TARGET_CLIENT)]
    pub async fn event_publisher<T, S>(&self) -> Result<EventPublisher<T, S>>
    where
        T: Serialize + Send + Sync + 'static,
        S: EventStream,
    {
        EventPublisher::new(&self.inner.jetstream).await
    }

    /// Create an event subscriber for the specified stream type.
    #[tracing::instrument(skip(self), target = TRACING_TARGET_CLIENT)]
    pub async fn event_subscriber<T, S>(&self) -> Result<EventSubscriber<T, S>>
    where
        T: DeserializeOwned + Send + Sync + 'static,
        S: EventStream,
    {
        EventSubscriber::new(&self.inner.jetstream).await
    }

    /// Create a file job publisher.
    #[tracing::instrument(skip(self), target = TRACING_TARGET_CLIENT)]
    pub async fn file_publisher<T>(&self) -> Result<EventPublisher<T, FileStream>>
    where
        T: Serialize + Send + Sync + 'static,
    {
        self.event_publisher().await
    }

    /// Create a file job subscriber.
    #[tracing::instrument(skip(self), target = TRACING_TARGET_CLIENT)]
    pub async fn file_subscriber<T>(&self) -> Result<EventSubscriber<T, FileStream>>
    where
        T: DeserializeOwned + Send + Sync + 'static,
    {
        self.event_subscriber().await
    }

    /// Create a webhook publisher.
    #[tracing::instrument(skip(self), target = TRACING_TARGET_CLIENT)]
    pub async fn webhook_publisher<T>(&self) -> Result<EventPublisher<T, WebhookStream>>
    where
        T: Serialize + Send + Sync + 'static,
    {
        self.event_publisher().await
    }

    /// Create a webhook subscriber.
    #[tracing::instrument(skip(self), target = TRACING_TARGET_CLIENT)]
    pub async fn webhook_subscriber<T>(&self) -> Result<EventSubscriber<T, WebhookStream>>
    where
        T: DeserializeOwned + Send + Sync + 'static,
    {
        self.event_subscriber().await
    }
}
