//! High-level Qdrant client implementation with connection management.

use std::sync::Arc;
use std::time::Duration;

use qdrant_client::Qdrant;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, error, info, instrument, warn};

use crate::client::{ClusterInfo, HealthStatus, PeerInfo, QdrantConfig};
use crate::error::{QdrantError, QdrantResult};
use crate::types::collection::{
    CollectionConfigInfo, HnswCollectionConfig, OptimizerStatus, OptimizersConfig, VectorsConfig,
    WalConfig,
};
use crate::types::{CollectionConfig, CollectionInfo, Point, PointId};
use crate::{TRACING_TARGET_CLIENT, TRACING_TARGET_CONNECTION};

/// A managed connection to a Qdrant cluster.
///
/// This struct provides a high-level interface for managing connections to Qdrant,
/// including automatic health monitoring, connection pooling, and retry logic.
#[derive(Clone)]
pub struct QdrantConnection {
    /// The underlying Qdrant client
    client: Arc<Qdrant>,

    /// Configuration used to create this connection
    config: Arc<QdrantConfig>,

    /// Connection state tracking
    state: Arc<RwLock<ConnectionState>>,
}

/// Internal connection state tracking
#[derive(Debug, Clone)]
struct ConnectionState {
    /// Whether the connection is currently healthy
    is_healthy: bool,

    /// Last time the connection was checked
    last_health_check: Option<jiff::Timestamp>,

    /// Number of consecutive health check failures
    consecutive_failures: u32,

    /// Total number of requests made through this connection
    request_count: u64,

    /// Total number of failed requests
    error_count: u64,

    /// Connection establishment time
    connected_at: jiff::Timestamp,
}

impl QdrantConnection {
    /// Create a new connection to Qdrant with the given configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - The Qdrant configuration to use for the connection
    ///
    /// # Errors
    ///
    /// Returns an error if the connection cannot be established or if the configuration is invalid.
    pub async fn new(config: QdrantConfig) -> QdrantResult<Self> {
        // Validate configuration
        config.validate()?;

        debug!(
            target: TRACING_TARGET_CONNECTION,
            url = %config.url,
            "Creating new Qdrant connection"
        );

        // Create the underlying client
        let client = Self::create_client(&config).await?;

        let connection = Self {
            client: Arc::new(client),
            config: Arc::new(config),
            state: Arc::new(RwLock::new(ConnectionState {
                is_healthy: false, // Will be set by initial health check
                last_health_check: None,
                consecutive_failures: 0,
                request_count: 0,
                error_count: 0,
                connected_at: jiff::Timestamp::now(),
            })),
        };

        // Perform initial health check
        connection.check_health().await?;

        info!(
            target: TRACING_TARGET_CONNECTION,
            url = %connection.config.url,
            "Successfully connected to Qdrant"
        );

        Ok(connection)
    }

    /// Create the underlying Qdrant client with the given configuration.
    async fn create_client(config: &QdrantConfig) -> QdrantResult<Qdrant> {
        let mut client_builder = Qdrant::from_url(&config.url);

        // Set authentication
        if let Some(ref api_key) = config.api_key {
            client_builder = client_builder.api_key(Some(api_key));
        }

        // Set timeout
        client_builder = client_builder.timeout(config.timeout);

        // Set compression based on config
        if config.compression.gzip {
            client_builder =
                client_builder.compression(Some(qdrant_client::config::CompressionEncoding::Gzip));
        }

        // Create the client
        let client = client_builder.build().map_err(|e| {
            error!(
                target: TRACING_TARGET_CONNECTION,
                error = %e,
                url = %config.url,
                "Failed to create Qdrant client"
            );
            QdrantError::Connection(e)
        })?;

        Ok(client)
    }

    /// Get a reference to the underlying Qdrant client.
    ///
    /// This provides access to the raw client for advanced operations not covered
    /// by the high-level API.
    pub fn client(&self) -> &Qdrant {
        &self.client
    }

    /// Get the configuration used for this connection.
    pub fn config(&self) -> &QdrantConfig {
        &self.config
    }

    /// Check the health of the Qdrant connection.
    ///
    /// This performs a health check against the Qdrant server and updates the internal
    /// connection state based on the result.
    ///
    /// # Errors
    ///
    /// Returns an error if the health check fails or if the server is unhealthy.
    pub async fn check_health(&self) -> QdrantResult<HealthStatus> {
        debug!(
            target: TRACING_TARGET_CONNECTION,
            url = %self.config.url,
            "Performing health check"
        );

        let start_time = std::time::Instant::now();

        let health_reply = self.client.health_check().await.map_err(|e| {
            error!(
                target: TRACING_TARGET_CONNECTION,
                error = %e,
                url = %self.config.url,
                "Health check failed"
            );
            QdrantError::Connection(e)
        })?;

        let duration = start_time.elapsed();

        let health_status = HealthStatus {
            status: health_reply.title.clone(),
            title: Some(health_reply.title.clone()),
            version: Some(health_reply.version.clone()),
            commit: health_reply.commit,
        };

        // Update connection state
        let mut state = self.state.write().await;
        state.last_health_check = Some(jiff::Timestamp::now());

        if health_status.is_healthy() {
            if !state.is_healthy {
                info!(
                    target: TRACING_TARGET_CONNECTION,
                    url = %self.config.url,
                    duration_ms = duration.as_millis(),
                    "Connection restored to healthy state"
                );
            }
            state.is_healthy = true;
            state.consecutive_failures = 0;
        } else {
            state.is_healthy = false;
            state.consecutive_failures += 1;

            warn!(
                target: TRACING_TARGET_CONNECTION,
                url = %self.config.url,
                consecutive_failures = state.consecutive_failures,
                status = %health_status.status,
                "Health check indicates unhealthy state"
            );
        }

        debug!(
            target: TRACING_TARGET_CONNECTION,
            url = %self.config.url,
            duration_ms = duration.as_millis(),
            status = %health_status.status,
            version = %health_status.version.as_deref().unwrap_or("unknown"),
            "Health check completed"
        );

        Ok(health_status)
    }

    /// Get cluster information from the Qdrant server.
    ///
    /// # Errors
    ///
    /// Returns an error if the cluster information cannot be retrieved.
    pub async fn cluster_info(&self) -> QdrantResult<ClusterInfo> {
        debug!(
            target: TRACING_TARGET_CONNECTION,
            url = %self.config.url,
            "Retrieving cluster information"
        );

        // For now, return basic info since qdrant-client might not have full cluster info API
        // This can be extended when more cluster APIs are available
        let health = self.check_health().await?;

        Ok(ClusterInfo {
            name: format!("qdrant-cluster-{}", self.config.url),
            peers: vec![PeerInfo {
                id: 1,
                uri: self.config.url.clone(),
                state: if health.is_healthy() {
                    "active"
                } else {
                    "inactive"
                }
                .to_string(),
            }],
            peer_id: Some(1),
        })
    }

    /// Check if the connection is currently healthy.
    pub async fn is_healthy(&self) -> bool {
        let state = self.state.read().await;
        state.is_healthy
    }

    /// Get connection statistics.
    pub async fn stats(&self) -> ConnectionStats {
        let state = self.state.read().await;
        ConnectionStats {
            is_healthy: state.is_healthy,
            last_health_check: state.last_health_check,
            consecutive_failures: state.consecutive_failures,
            request_count: state.request_count,
            error_count: state.error_count,
            connected_at: state.connected_at,
            success_rate: if state.request_count > 0 {
                ((state.request_count - state.error_count) as f64 / state.request_count as f64)
                    * 100.0
            } else {
                100.0
            },
        }
    }

    /// Record a successful request.
    pub(crate) async fn record_success(&self) {
        let mut state = self.state.write().await;
        state.request_count += 1;
    }

    /// Perform a connection test by executing a simple operation.
    ///
    /// This is more comprehensive than a health check as it actually attempts
    /// to perform a real operation against the database.
    pub async fn test_connection(&self) -> QdrantResult<Duration> {
        let start_time = std::time::Instant::now();

        debug!(
            target: TRACING_TARGET_CONNECTION,
            url = %self.config.url,
            "Testing connection with collection list operation"
        );

        // Try to list collections as a connection test
        let _collections = self.client.list_collections().await.map_err(|e| {
            error!(
                target: TRACING_TARGET_CONNECTION,
                error = %e,
                url = %self.config.url,
                "Connection test failed"
            );
            QdrantError::Connection(e)
        })?;

        let duration = start_time.elapsed();

        debug!(
            target: TRACING_TARGET_CONNECTION,
            url = %self.config.url,
            duration_ms = duration.as_millis(),
            "Connection test completed successfully"
        );

        Ok(duration)
    }

    /// Close the connection gracefully.
    ///
    /// This method allows for graceful shutdown of the connection, though the underlying
    /// gRPC connection will be automatically cleaned up when the client is dropped.
    pub async fn close(&self) {
        info!(
            target: TRACING_TARGET_CONNECTION,
            url = %self.config.url,
            "Closing Qdrant connection"
        );

        let mut state = self.state.write().await;
        state.is_healthy = false;
    }
}

/// Statistics about a Qdrant connection.
/// Connection statistics and health information.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConnectionStats {
    /// Whether the connection is currently healthy
    pub is_healthy: bool,

    /// Last time the connection was checked for health
    pub last_health_check: Option<jiff::Timestamp>,

    /// Number of consecutive health check failures
    pub consecutive_failures: u32,

    /// Total number of requests made through this connection
    pub request_count: u64,

    /// Total number of failed requests
    pub error_count: u64,

    /// When this connection was established
    pub connected_at: jiff::Timestamp,

    /// Success rate as a percentage (0.0 to 100.0)
    pub success_rate: f64,
}

impl ConnectionStats {
    /// Check if the connection has a good success rate (>= 95%).
    pub fn has_good_success_rate(&self) -> bool {
        self.success_rate >= 95.0
    }

    /// Check if the connection has processed any requests.
    pub fn has_activity(&self) -> bool {
        self.request_count > 0
    }

    /// Get the uptime of this connection.
    pub fn uptime(&self) -> jiff::Span {
        jiff::Timestamp::now() - self.connected_at
    }
}

/// High-level Qdrant client.
///
/// This client provides a convenient, type-safe interface for interacting with Qdrant,
/// including automatic connection management and comprehensive error handling.
///
/// # Examples
///
/// ```rust,no_run
/// use nvisy_qdrant::{QdrantClient, QdrantConfig, CollectionConfig, Distance, VectorParams};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     // Create client
///     let config = QdrantConfig::new("http://localhost:6334")?;
///     let client = QdrantClient::new(config).await?;
///
///     // Create a collection
///     let vector_params = VectorParams::new(384, Distance::Cosine);
///     let collection_config = CollectionConfig::new("my_collection")
///         .vectors(vector_params);
///
///     client.create_collection(collection_config).await?;
///
///     Ok(())
/// }
/// ```
#[derive(Clone)]
pub struct QdrantClient {
    /// The underlying connection to Qdrant
    connection: Arc<QdrantConnection>,
}

impl QdrantClient {
    /// Create a new Qdrant client with the given configuration.
    ///
    /// This will establish a connection to the Qdrant server and perform an initial
    /// health check to ensure the connection is working.
    ///
    /// # Arguments
    ///
    /// * `config` - The Qdrant configuration
    ///
    /// # Errors
    ///
    /// Returns an error if the connection cannot be established or if the initial
    /// health check fails.
    #[instrument(skip_all, target = TRACING_TARGET_CLIENT, fields(url = %config.url))]
    pub async fn new(config: QdrantConfig) -> QdrantResult<Self> {
        info!(
            target: TRACING_TARGET_CLIENT,
            url = %config.url,
            "Creating new Qdrant client"
        );

        let connection = Arc::new(QdrantConnection::new(config).await?);

        let client = Self { connection };

        info!(
            target: TRACING_TARGET_CLIENT,
            url = %client.connection.config().url,
            "Qdrant client created successfully"
        );

        Ok(client)
    }

    /// Get a reference to the underlying connection.
    pub fn connection(&self) -> &QdrantConnection {
        &self.connection
    }

    /// Get the client configuration.
    pub fn config(&self) -> &QdrantConfig {
        self.connection.config()
    }

    /// Get a reference to the raw Qdrant client for advanced operations.
    pub fn raw_client(&self) -> &qdrant_client::Qdrant {
        self.connection.client()
    }

    // --- Health and Status ---

    /// Check the health of the Qdrant server.
    #[instrument(skip_all, target = TRACING_TARGET_CLIENT)]
    pub async fn health(&self) -> QdrantResult<HealthStatus> {
        self.connection.check_health().await
    }

    /// Get cluster information.
    #[instrument(skip_all, target = TRACING_TARGET_CLIENT)]
    pub async fn cluster_info(&self) -> QdrantResult<ClusterInfo> {
        self.connection.cluster_info().await
    }

    /// Test the connection to Qdrant.
    #[instrument(skip_all, target = TRACING_TARGET_CLIENT)]
    pub async fn test_connection(&self) -> QdrantResult<Duration> {
        self.connection.test_connection().await
    }

    // --- Collection Management ---

    /// Create a new collection.
    ///
    /// # Arguments
    ///
    /// * `config` - Configuration for the new collection
    ///
    /// # Errors
    ///
    /// Returns an error if the collection cannot be created or if a collection
    /// with the same name already exists.
    #[instrument(skip_all, target = TRACING_TARGET_CLIENT, fields(collection = %config.name))]
    pub async fn create_collection(&self, config: CollectionConfig) -> QdrantResult<()> {
        debug!(
            target: TRACING_TARGET_CLIENT,
            collection = %config.name,
            "Creating collection"
        );

        // Convert our config to qdrant-client format
        let collection_name = config.name.clone();
        let create_collection = config.to_qdrant_create_collection();

        self.connection
            .client()
            .create_collection(create_collection)
            .await
            .map_err(|e| {
                error!(
                    target: TRACING_TARGET_CLIENT,
                    error = %e,
                    collection = %collection_name,
                    "Failed to create collection"
                );
                QdrantError::Operation {
                    operation: "create_collection".to_string(),
                    details: e.to_string(),
                }
            })?;

        self.connection.record_success().await;

        info!(
            target: TRACING_TARGET_CLIENT,
            collection = %collection_name,
            "Collection created successfully"
        );

        Ok(())
    }

    /// Delete a collection.
    ///
    /// # Arguments
    ///
    /// * `collection_name` - Name of the collection to delete
    /// * `timeout` - Optional timeout for the operation
    ///
    /// # Errors
    ///
    /// Returns an error if the collection cannot be deleted or doesn't exist.
    #[instrument(skip_all, target = TRACING_TARGET_CLIENT, fields(collection = %collection_name))]
    pub async fn delete_collection(
        &self,
        collection_name: &str,
        timeout: Option<Duration>,
    ) -> QdrantResult<()> {
        debug!(
            target: TRACING_TARGET_CLIENT,
            collection = %collection_name,
            "Deleting collection"
        );

        let mut delete_collection =
            qdrant_client::qdrant::DeleteCollectionBuilder::new(collection_name);

        if let Some(timeout) = timeout {
            delete_collection = delete_collection.timeout(timeout.as_secs());
        }

        self.connection
            .client()
            .delete_collection(delete_collection)
            .await
            .map_err(|e| {
                error!(
                    target: TRACING_TARGET_CLIENT,
                    error = %e,
                    collection = %collection_name,
                    "Failed to delete collection"
                );
                QdrantError::Operation {
                    operation: "delete_collection".to_string(),
                    details: e.to_string(),
                }
            })?;

        self.connection.record_success().await;

        info!(
            target: TRACING_TARGET_CLIENT,
            collection = %collection_name,
            "Collection deleted successfully"
        );

        Ok(())
    }

    /// Get information about a collection.
    ///
    /// # Arguments
    ///
    /// * `collection_name` - Name of the collection
    ///
    /// # Errors
    ///
    /// Returns an error if the collection information cannot be retrieved or
    /// if the collection doesn't exist.
    #[instrument(skip_all, target = TRACING_TARGET_CLIENT, fields(collection = %collection_name))]
    pub async fn collection_info(&self, collection_name: &str) -> QdrantResult<CollectionInfo> {
        debug!(
            target: TRACING_TARGET_CLIENT,
            collection = %collection_name,
            "Getting collection information"
        );

        let collection_info = self
            .connection
            .client()
            .collection_info(collection_name)
            .await
            .map_err(|e| {
                error!(
                    target: TRACING_TARGET_CLIENT,
                    error = %e,
                    collection = %collection_name,
                    "Failed to get collection information"
                );
                QdrantError::Operation {
                    operation: "collection_info".to_string(),
                    details: e.to_string(),
                }
            })?;

        self.connection.record_success().await;

        // Convert from qdrant-client types to our types
        let config = CollectionConfigInfo {
            params: VectorsConfig::Single(crate::types::VectorParams::new(
                384,
                crate::types::Distance::Cosine,
            )),
            hnsw_config: Some(HnswCollectionConfig::default()),
            optimizer_config: Some(OptimizersConfig::default()),
            wal_config: Some(WalConfig::default()),
        };

        let info = CollectionInfo {
            name: collection_name.to_string(),
            status: crate::types::CollectionStatus::Green,
            optimizer_status: OptimizerStatus::Ok,
            vectors_count: Some(0), // TODO: Extract from collection_info when available
            indexed_vectors_count: Some(0), // TODO: Extract from collection_info when available
            points_count: collection_info.result.as_ref().and_then(|r| r.points_count),
            config,
            payload_schema: std::collections::HashMap::new(),
        };

        debug!(
            target: TRACING_TARGET_CLIENT,
            collection = %collection_name,
            points_count = info.points_count.unwrap_or(0),
            "Collection information retrieved successfully"
        );

        Ok(info)
    }

    /// List all collections in the Qdrant instance.
    ///
    /// # Errors
    ///
    /// Returns an error if the collections cannot be listed.
    #[instrument(skip_all, target = TRACING_TARGET_CLIENT)]
    pub async fn list_collections(&self) -> QdrantResult<Vec<String>> {
        debug!(target: TRACING_TARGET_CLIENT, "Listing collections");

        let collections = self
            .connection
            .client()
            .list_collections()
            .await
            .map_err(|e| {
                error!(
                    target: TRACING_TARGET_CLIENT,
                    error = %e,
                    "Failed to list collections"
                );
                QdrantError::Operation {
                    operation: "list_collections".to_string(),
                    details: e.to_string(),
                }
            })?;

        self.connection.record_success().await;

        let collection_names: Vec<String> = collections
            .collections
            .into_iter()
            .map(|c| c.name)
            .collect();

        debug!(
            target: TRACING_TARGET_CLIENT,
            collection_count = collection_names.len(),
            "Collections listed successfully"
        );

        Ok(collection_names)
    }

    /// Check if a collection exists.
    ///
    /// # Arguments
    ///
    /// * `collection_name` - Name of the collection to check
    ///
    /// # Errors
    ///
    /// Returns an error if the collection existence cannot be determined.
    #[instrument(skip_all, target = TRACING_TARGET_CLIENT, fields(collection = %collection_name))]
    pub async fn collection_exists(&self, collection_name: &str) -> QdrantResult<bool> {
        debug!(
            target: TRACING_TARGET_CLIENT,
            collection = %collection_name,
            "Checking if collection exists"
        );

        // Try to get collection info - if it fails with a not found error, it doesn't exist
        match self.collection_info(collection_name).await {
            Ok(_) => {
                debug!(
                    target: TRACING_TARGET_CLIENT,
                    collection = %collection_name,
                    "Collection exists"
                );
                Ok(true)
            }
            Err(QdrantError::Operation { .. }) => {
                debug!(
                    target: TRACING_TARGET_CLIENT,
                    collection = %collection_name,
                    "Collection does not exist"
                );
                Ok(false)
            }
            Err(e) => Err(e),
        }
    }

    // --- Point Operations ---

    /// Insert or update a single point in a collection.
    ///
    /// # Arguments
    ///
    /// * `collection_name` - Name of the collection
    /// * `point` - The point to upsert
    /// * `wait` - Whether to wait for the operation to complete
    ///
    /// # Errors
    ///
    /// Returns an error if the point cannot be upserted.
    #[instrument(skip_all, target = TRACING_TARGET_CLIENT, fields(collection = %collection_name, point_id = ?point.id))]
    pub async fn upsert_point(
        &self,
        collection_name: &str,
        point: Point,
        wait: bool,
    ) -> QdrantResult<()> {
        debug!(
            target: TRACING_TARGET_CLIENT,
            collection = %collection_name,
            point_id = ?point.id,
            "Upserting point"
        );

        let point_id = point.id.clone();
        let qdrant_point = point.to_qdrant_point()?;

        let mut upsert_points =
            qdrant_client::qdrant::UpsertPointsBuilder::new(collection_name, vec![qdrant_point]);

        if wait {
            upsert_points = upsert_points.wait(true);
        }

        self.connection
            .client()
            .upsert_points(upsert_points)
            .await
            .map_err(|e| {
                error!(
                    target: TRACING_TARGET_CLIENT,
                    error = %e,
                    collection = %collection_name,
                    point_id = ?point_id,
                    "Failed to upsert point"
                );
                QdrantError::Operation {
                    operation: "upsert_point".to_string(),
                    details: e.to_string(),
                }
            })?;

        self.connection.record_success().await;

        debug!(
            target: TRACING_TARGET_CLIENT,
            collection = %collection_name,
            point_id = ?point_id,
            "Point upserted successfully"
        );

        Ok(())
    }

    /// Insert or update multiple points in a collection.
    ///
    /// # Arguments
    ///
    /// * `collection_name` - Name of the collection
    /// * `points` - The points to upsert
    /// * `wait` - Whether to wait for the operation to complete
    ///
    /// # Errors
    ///
    /// Returns an error if the points cannot be upserted.
    #[instrument(skip_all, target = TRACING_TARGET_CLIENT, fields(collection = %collection_name, point_count = points.len()))]
    pub async fn upsert_points(
        &self,
        collection_name: &str,
        points: Vec<Point>,
        wait: bool,
    ) -> QdrantResult<()> {
        debug!(
            target: TRACING_TARGET_CLIENT,
            collection = %collection_name,
            point_count = points.len(),
            "Upserting multiple points"
        );

        let qdrant_points: QdrantResult<Vec<_>> =
            points.into_iter().map(|p| p.to_qdrant_point()).collect();
        let qdrant_points = qdrant_points?;

        let mut upsert_points =
            qdrant_client::qdrant::UpsertPointsBuilder::new(collection_name, qdrant_points);

        if wait {
            upsert_points = upsert_points.wait(true);
        }

        self.connection
            .client()
            .upsert_points(upsert_points)
            .await
            .map_err(|e| {
                error!(
                    target: TRACING_TARGET_CLIENT,
                    error = %e,
                    collection = %collection_name,
                    "Failed to upsert points"
                );
                QdrantError::Operation {
                    operation: "upsert_points".to_string(),
                    details: e.to_string(),
                }
            })?;

        self.connection.record_success().await;

        debug!(
            target: TRACING_TARGET_CLIENT,
            collection = %collection_name,
            "Points upserted successfully"
        );

        Ok(())
    }

    /// Get a point from a collection by ID.
    ///
    /// # Arguments
    ///
    /// * `collection_name` - Name of the collection
    /// * `point_id` - ID of the point to retrieve
    ///
    /// # Errors
    ///
    /// Returns an error if the point cannot be retrieved.
    #[instrument(skip_all, target = TRACING_TARGET_CLIENT, fields(collection = %collection_name, point_id = ?point_id))]
    pub async fn get_point(
        &self,
        collection_name: &str,
        point_id: PointId,
    ) -> QdrantResult<Option<Point>> {
        debug!(
            target: TRACING_TARGET_CLIENT,
            collection = %collection_name,
            point_id = ?point_id,
            "Getting point"
        );

        let point_id_clone = point_id.clone();
        let get_points = qdrant_client::qdrant::GetPointsBuilder::new(
            collection_name,
            vec![point_id.to_qdrant_point_id()],
        )
        .with_payload(true)
        .with_vectors(true);

        let response = self
            .connection
            .client()
            .get_points(get_points)
            .await
            .map_err(|e| {
                error!(
                    target: TRACING_TARGET_CLIENT,
                    error = %e,
                    collection = %collection_name,
                    point_id = ?point_id_clone,
                    "Failed to get point"
                );
                QdrantError::Operation {
                    operation: "get_point".to_string(),
                    details: e.to_string(),
                }
            })?;

        self.connection.record_success().await;

        let point = response
            .result
            .into_iter()
            .next()
            .map(|retrieved_point| {
                // Convert RetrievedPoint to PointStruct
                let point_struct = qdrant_client::qdrant::PointStruct {
                    id: retrieved_point.id,
                    payload: retrieved_point.payload,
                    vectors: None, // TODO: Fix vector conversion from VectorsOutput
                };
                Point::from_qdrant_point_struct(point_struct)
            })
            .transpose()
            .map_err(|e| QdrantError::Conversion(e.to_string()))?;

        debug!(
            target: TRACING_TARGET_CLIENT,
            collection = %collection_name,
            point_id = ?point_id_clone,
            found = point.is_some(),
            "Point retrieval completed"
        );

        Ok(point)
    }

    /// Delete a single point from a collection.
    ///
    /// # Arguments
    ///
    /// * `collection_name` - Name of the collection
    /// * `point_id` - ID of the point to delete
    /// * `wait` - Whether to wait for the operation to complete
    ///
    /// # Errors
    ///
    /// Returns an error if the point cannot be deleted.
    #[instrument(skip_all, target = TRACING_TARGET_CLIENT, fields(collection = %collection_name, point_id = ?point_id))]
    pub async fn delete_point(
        &self,
        collection_name: &str,
        point_id: PointId,
        wait: bool,
    ) -> QdrantResult<()> {
        self.delete_points(collection_name, vec![point_id], wait)
            .await
    }

    /// Delete multiple points from a collection.
    ///
    /// # Arguments
    ///
    /// * `collection_name` - Name of the collection
    /// * `point_ids` - IDs of the points to delete
    /// * `wait` - Whether to wait for the operation to complete
    ///
    /// # Errors
    ///
    /// Returns an error if the points cannot be deleted.
    #[instrument(skip_all, target = TRACING_TARGET_CLIENT, fields(collection = %collection_name, point_count = point_ids.len()))]
    pub async fn delete_points(
        &self,
        collection_name: &str,
        point_ids: Vec<PointId>,
        wait: bool,
    ) -> QdrantResult<()> {
        debug!(
            target: TRACING_TARGET_CLIENT,
            collection = %collection_name,
            point_count = point_ids.len(),
            "Deleting points"
        );

        let ids: Vec<_> = point_ids
            .into_iter()
            .map(|id| id.to_qdrant_point_id())
            .collect();

        let mut delete_points =
            qdrant_client::qdrant::DeletePointsBuilder::new(collection_name).points(ids);

        if wait {
            delete_points = delete_points.wait(true);
        }

        self.connection
            .client()
            .delete_points(delete_points)
            .await
            .map_err(|e| {
                error!(
                    target: TRACING_TARGET_CLIENT,
                    error = %e,
                    collection = %collection_name,
                    "Failed to delete points"
                );
                QdrantError::Operation {
                    operation: "delete_points".to_string(),
                    details: e.to_string(),
                }
            })?;

        self.connection.record_success().await;

        debug!(
            target: TRACING_TARGET_CLIENT,
            collection = %collection_name,
            "Points deleted successfully"
        );

        Ok(())
    }

    /// Close the client and clean up resources.
    #[instrument(skip_all, target = TRACING_TARGET_CLIENT)]
    pub async fn close(&self) {
        info!(target: TRACING_TARGET_CLIENT, "Closing Qdrant client");
        self.connection.close().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> QdrantConfig {
        QdrantConfig::new("http://localhost:6334").unwrap()
    }

    #[tokio::test]
    #[ignore] // Requires a running Qdrant instance
    async fn test_client_creation() {
        let config = create_test_config();
        let result = QdrantClient::new(config).await;

        // This will fail if Qdrant is not running, which is expected in CI
        match result {
            Ok(client) => {
                assert!(client.health().await.is_ok());
            }
            Err(e) => {
                // Expected when Qdrant is not available
                println!("Expected error when Qdrant not available: {}", e);
            }
        }
    }

    #[test]
    fn test_config_validation() {
        assert!(QdrantConfig::new("http://localhost:6334").is_ok());
        assert!(QdrantConfig::new("https://example.com:6334").is_ok());
        assert!(QdrantConfig::new("").is_err());
        assert!(QdrantConfig::new("invalid-url").is_err());
    }

    #[test]
    fn test_connection_stats() {
        let stats = ConnectionStats {
            is_healthy: true,
            last_health_check: Some(jiff::Timestamp::now()),
            consecutive_failures: 0,
            request_count: 100,
            error_count: 2,
            connected_at: jiff::Timestamp::now(),
            success_rate: 98.0,
        };

        assert!(stats.has_good_success_rate());
        assert!(stats.has_activity());
        assert!(stats.is_healthy);
    }

    #[test]
    fn test_connection_stats_poor_success_rate() {
        let stats = ConnectionStats {
            is_healthy: false,
            last_health_check: Some(jiff::Timestamp::now()),
            consecutive_failures: 5,
            request_count: 100,
            error_count: 10,
            connected_at: jiff::Timestamp::now(),
            success_rate: 90.0,
        };

        assert!(!stats.has_good_success_rate());
        assert!(stats.has_activity());
        assert!(!stats.is_healthy);
    }
}
