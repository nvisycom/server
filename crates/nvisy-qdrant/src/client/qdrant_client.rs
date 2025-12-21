//! High-level Qdrant client implementation with connection management.

use std::sync::Arc;
use std::time::Duration;

use qdrant_client::Qdrant;
use tracing::{debug, error, info, instrument};

use crate::TRACING_TARGET_CLIENT;
use crate::client::QdrantConfig;
use crate::error::{Error, Result};
use crate::types::{Point, PointId};

/// High-level Qdrant client.
///
/// This client provides a convenient, type-safe interface for interacting with Qdrant,
/// including automatic connection management and comprehensive error handling.
///
/// # Examples
///
/// ```rust,no_run
/// use nvisy_qdrant::{QdrantClient, QdrantConfig, AnnotationCollection};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     // Create client
///     let config = QdrantConfig::new("http://localhost:6334")?;
///     let client = QdrantClient::new(config).await?;
///
///     // Create a collection using the collection trait
///     client.create_collection(384).await?;
///
///     Ok(())
/// }
/// ```
#[derive(Clone)]
pub struct QdrantClient {
    /// The underlying client state
    inner: Arc<QdrantClientInner>,
}

/// Internal client state.
struct QdrantClientInner {
    /// The underlying Qdrant client
    client: Qdrant,

    /// Configuration used to create this client
    config: QdrantConfig,
}

impl QdrantClient {
    /// Create a new Qdrant client with the given configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - The Qdrant configuration
    ///
    /// # Errors
    ///
    /// Returns an error if the connection cannot be established.
    #[instrument(skip_all, target = TRACING_TARGET_CLIENT, fields(url = %config.url))]
    pub async fn new(config: QdrantConfig) -> Result<Self> {
        // Validate configuration
        config.validate()?;

        info!(
            target: TRACING_TARGET_CLIENT,
            url = %config.url,
            "Creating new Qdrant client"
        );

        // Build the Qdrant client
        let mut client_builder = Qdrant::from_url(&config.url);

        if let Some(ref api_key) = config.api_key {
            client_builder = client_builder.api_key(Some(api_key));
        }

        client_builder = client_builder.timeout(config.timeout);

        let client = client_builder.build().map_err(|e| {
            error!(
                target: TRACING_TARGET_CLIENT,
                error = %e,
                url = %config.url,
                "Failed to create Qdrant client"
            );
            Error::connection().with_source(Box::new(e))
        })?;

        let inner = Arc::new(QdrantClientInner { client, config });

        info!(
            target: TRACING_TARGET_CLIENT,
            url = %inner.config.url,
            "Qdrant client created successfully"
        );

        Ok(Self { inner })
    }

    /// Get the client configuration.
    pub fn config(&self) -> &QdrantConfig {
        &self.inner.config
    }

    /// Get a reference to the raw Qdrant client for advanced operations.
    pub fn raw_client(&self) -> &qdrant_client::Qdrant {
        &self.inner.client
    }

    /// Create a new collection.
    ///
    /// # Arguments
    ///
    /// * `create_collection` - Qdrant CreateCollection request
    ///
    /// # Errors
    ///
    /// Returns an error if the collection cannot be created or if a collection
    /// with the same name already exists.
    #[instrument(skip_all, target = TRACING_TARGET_CLIENT, fields(collection = %create_collection.collection_name))]
    pub(crate) async fn create_collection(
        &self,
        create_collection: qdrant_client::qdrant::CreateCollection,
    ) -> Result<()> {
        let collection_name = create_collection.collection_name.clone();

        debug!(
            target: TRACING_TARGET_CLIENT,
            collection = %collection_name,
            "Creating collection"
        );

        self.inner
            .client
            .create_collection(create_collection)
            .await
            .map_err(|e| {
                error!(
                    target: TRACING_TARGET_CLIENT,
                    error = %e,
                    collection = %collection_name,
                    "Failed to create collection"
                );
                Error::collection().with_message(format!("create_collection failed: {}", e))
            })?;

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
    pub(crate) async fn delete_collection(
        &self,
        collection_name: &str,
        timeout: Option<Duration>,
    ) -> Result<()> {
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

        self.inner
            .client
            .delete_collection(delete_collection)
            .await
            .map_err(|e| {
                error!(
                    target: TRACING_TARGET_CLIENT,
                    error = %e,
                    collection = %collection_name,
                    "Failed to delete collection"
                );
                Error::collection().with_message(format!("delete_collection failed: {}", e))
            })?;

        info!(
            target: TRACING_TARGET_CLIENT,
            collection = %collection_name,
            "Collection deleted successfully"
        );

        Ok(())
    }

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
    pub(crate) async fn upsert_point(
        &self,
        collection_name: &str,
        point: Point,
        wait: bool,
    ) -> Result<()> {
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

        self.inner
            .client
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
                Error::point().with_message(format!("upsert_point failed: {}", e))
            })?;

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
    pub(crate) async fn upsert_points(
        &self,
        collection_name: &str,
        points: Vec<Point>,
        wait: bool,
    ) -> Result<()> {
        debug!(
            target: TRACING_TARGET_CLIENT,
            collection = %collection_name,
            point_count = points.len(),
            "Upserting multiple points"
        );

        let qdrant_points: Result<Vec<_>> =
            points.into_iter().map(|p| p.to_qdrant_point()).collect();
        let qdrant_points = qdrant_points?;

        let mut upsert_points =
            qdrant_client::qdrant::UpsertPointsBuilder::new(collection_name, qdrant_points);

        if wait {
            upsert_points = upsert_points.wait(true);
        }

        self.inner
            .client
            .upsert_points(upsert_points)
            .await
            .map_err(|e| {
                error!(
                    target: TRACING_TARGET_CLIENT,
                    error = %e,
                    collection = %collection_name,
                    "Failed to upsert points"
                );
                Error::point().with_message(format!("upsert_points failed: {}", e))
            })?;

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
    pub(crate) async fn get_point(
        &self,
        collection_name: &str,
        point_id: PointId,
    ) -> Result<Option<Point>> {
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
            .inner
            .client
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
                Error::point().with_message(format!("get_point failed: {}", e))
            })?;

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
            .map_err(|e| Error::serialization().with_message(e.to_string()))?;

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
    pub(crate) async fn delete_point(
        &self,
        collection_name: &str,
        point_id: PointId,
        wait: bool,
    ) -> Result<()> {
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
    pub(crate) async fn delete_points(
        &self,
        collection_name: &str,
        point_ids: Vec<PointId>,
        wait: bool,
    ) -> Result<()> {
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

        self.inner
            .client
            .delete_points(delete_points)
            .await
            .map_err(|e| {
                error!(
                    target: TRACING_TARGET_CLIENT,
                    error = %e,
                    collection = %collection_name,
                    "Failed to delete points"
                );
                Error::point().with_message(format!("delete_points failed: {}", e))
            })?;

        debug!(
            target: TRACING_TARGET_CLIENT,
            collection = %collection_name,
            "Points deleted successfully"
        );

        Ok(())
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
            Ok(_client) => {
                // Client created successfully
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
}
