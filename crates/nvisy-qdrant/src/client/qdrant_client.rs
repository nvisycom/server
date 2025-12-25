//! High-level Qdrant client implementation with connection management.

use std::sync::Arc;

use qdrant_client::Qdrant;
use qdrant_client::qdrant::{
    CreateCollection, DeleteCollectionBuilder, DeletePointsBuilder, GetPointsBuilder,
    UpsertPointsBuilder,
};

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
///     let config = QdrantConfig::new("http://localhost:6334", "your-api-key")?;
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
    /// Returns an error if the configuration is invalid or client creation fails.
    #[tracing::instrument(skip_all, target = TRACING_TARGET_CLIENT, fields(url = %config.url))]
    pub fn new(config: QdrantConfig) -> Result<Self> {
        config.validate()?;

        tracing::info!(
            target: TRACING_TARGET_CLIENT,
            url = %config.url,
            "Creating new Qdrant client"
        );

        let mut builder = Qdrant::from_url(&config.url).api_key(config.api_key.as_str());

        if let Some(connect_timeout) = config.connect_timeout() {
            builder.set_connect_timeout(connect_timeout);
        }

        if let Some(timeout) = config.timeout() {
            builder.set_timeout(timeout);
        }

        if let Some(keep_alive) = config.keep_alive {
            builder.set_keep_alive_while_idle(keep_alive);
        }

        let client = builder.build().map_err(|e| {
            tracing::error!(
                target: TRACING_TARGET_CLIENT,
                error = %e,
                url = %config.url,
                "Failed to create Qdrant client"
            );
            Error::connection().with_source(Box::new(e))
        })?;

        let inner = Arc::new(QdrantClientInner { client, config });

        tracing::info!(
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
    #[tracing::instrument(skip_all, target = TRACING_TARGET_CLIENT, fields(collection = %create_collection.collection_name))]
    pub(crate) async fn create_collection(
        &self,
        create_collection: CreateCollection,
    ) -> Result<()> {
        let collection_name = create_collection.collection_name.clone();

        tracing::debug!(
            target: TRACING_TARGET_CLIENT,
            collection = %collection_name,
            "Creating collection"
        );

        self.inner
            .client
            .create_collection(create_collection)
            .await
            .map_err(|e| {
                tracing::error!(
                    target: TRACING_TARGET_CLIENT,
                    error = %e,
                    collection = %collection_name,
                    "Failed to create collection"
                );
                Error::collection().with_message(format!("create_collection failed: {}", e))
            })?;

        tracing::info!(
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
    ///
    /// # Errors
    ///
    /// Returns an error if the collection cannot be deleted or doesn't exist.
    #[tracing::instrument(skip_all, target = TRACING_TARGET_CLIENT, fields(collection = %collection_name))]
    pub(crate) async fn delete_collection(&self, collection_name: &str) -> Result<()> {
        tracing::debug!(
            target: TRACING_TARGET_CLIENT,
            collection = %collection_name,
            "Deleting collection"
        );

        let mut delete_collection = DeleteCollectionBuilder::new(collection_name);

        if let Some(timeout) = self.inner.config.timeout() {
            delete_collection = delete_collection.timeout(timeout.as_secs());
        }

        self.inner
            .client
            .delete_collection(delete_collection)
            .await
            .map_err(|e| {
                tracing::error!(
                    target: TRACING_TARGET_CLIENT,
                    error = %e,
                    collection = %collection_name,
                    "Failed to delete collection"
                );
                Error::collection().with_message(format!("delete_collection failed: {}", e))
            })?;

        tracing::info!(
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
    ///
    /// # Errors
    ///
    /// Returns an error if the point cannot be upserted.
    #[tracing::instrument(skip_all, target = TRACING_TARGET_CLIENT, fields(collection = %collection_name, point_id = ?point.id))]
    pub(crate) async fn upsert_point(&self, collection_name: &str, point: Point) -> Result<()> {
        tracing::debug!(
            target: TRACING_TARGET_CLIENT,
            collection = %collection_name,
            point_id = ?point.id,
            "Upserting point"
        );

        let point_id = point.id.clone();
        let qdrant_point = point.to_qdrant_point()?;

        let upsert_points =
            UpsertPointsBuilder::new(collection_name, vec![qdrant_point]).wait(true);

        self.inner
            .client
            .upsert_points(upsert_points)
            .await
            .map_err(|e| {
                tracing::error!(
                    target: TRACING_TARGET_CLIENT,
                    error = %e,
                    collection = %collection_name,
                    point_id = ?point_id,
                    "Failed to upsert point"
                );
                Error::point().with_message(format!("upsert_point failed: {}", e))
            })?;

        tracing::debug!(
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
    ///
    /// # Errors
    ///
    /// Returns an error if the points cannot be upserted.
    #[tracing::instrument(skip_all, target = TRACING_TARGET_CLIENT, fields(collection = %collection_name, point_count = points.len()))]
    pub(crate) async fn upsert_points(
        &self,
        collection_name: &str,
        points: Vec<Point>,
    ) -> Result<()> {
        tracing::debug!(
            target: TRACING_TARGET_CLIENT,
            collection = %collection_name,
            point_count = points.len(),
            "Upserting multiple points"
        );

        let qdrant_points: Result<Vec<_>> =
            points.into_iter().map(|p| p.to_qdrant_point()).collect();
        let qdrant_points = qdrant_points?;

        let upsert_points = UpsertPointsBuilder::new(collection_name, qdrant_points).wait(true);

        self.inner
            .client
            .upsert_points(upsert_points)
            .await
            .map_err(|e| {
                tracing::error!(
                    target: TRACING_TARGET_CLIENT,
                    error = %e,
                    collection = %collection_name,
                    "Failed to upsert points"
                );
                Error::point().with_message(format!("upsert_points failed: {}", e))
            })?;

        tracing::debug!(
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
    #[tracing::instrument(skip_all, target = TRACING_TARGET_CLIENT, fields(collection = %collection_name, point_id = ?point_id))]
    pub(crate) async fn get_point(
        &self,
        collection_name: &str,
        point_id: PointId,
    ) -> Result<Option<Point>> {
        tracing::debug!(
            target: TRACING_TARGET_CLIENT,
            collection = %collection_name,
            point_id = ?point_id,
            "Getting point"
        );

        let point_id_clone = point_id.clone();
        let mut get_points =
            GetPointsBuilder::new(collection_name, vec![point_id.to_qdrant_point_id()])
                .with_payload(true)
                .with_vectors(true);

        if let Some(timeout) = self.inner.config.timeout() {
            get_points = get_points.timeout(timeout.as_secs());
        }

        let response = self
            .inner
            .client
            .get_points(get_points)
            .await
            .map_err(|e| {
                tracing::error!(
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
            .map(Point::try_from)
            .transpose()
            .map_err(|e| Error::serialization().with_message(e.to_string()))?;

        tracing::debug!(
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
    ///
    /// # Errors
    ///
    /// Returns an error if the point cannot be deleted.
    #[tracing::instrument(skip_all, target = TRACING_TARGET_CLIENT, fields(collection = %collection_name, point_id = ?point_id))]
    pub(crate) async fn delete_point(
        &self,
        collection_name: &str,
        point_id: PointId,
    ) -> Result<()> {
        self.delete_points(collection_name, vec![point_id]).await
    }

    /// Delete multiple points from a collection.
    ///
    /// # Arguments
    ///
    /// * `collection_name` - Name of the collection
    /// * `point_ids` - IDs of the points to delete
    ///
    /// # Errors
    ///
    /// Returns an error if the points cannot be deleted.
    #[tracing::instrument(skip_all, target = TRACING_TARGET_CLIENT, fields(collection = %collection_name, point_count = point_ids.len()))]
    pub(crate) async fn delete_points(
        &self,
        collection_name: &str,
        point_ids: Vec<PointId>,
    ) -> Result<()> {
        tracing::debug!(
            target: TRACING_TARGET_CLIENT,
            collection = %collection_name,
            point_count = point_ids.len(),
            "Deleting points"
        );

        let ids: Vec<_> = point_ids
            .into_iter()
            .map(|id| id.to_qdrant_point_id())
            .collect();

        let delete_points = DeletePointsBuilder::new(collection_name)
            .points(ids)
            .wait(true);

        self.inner
            .client
            .delete_points(delete_points)
            .await
            .map_err(|e| {
                tracing::error!(
                    target: TRACING_TARGET_CLIENT,
                    error = %e,
                    collection = %collection_name,
                    "Failed to delete points"
                );
                Error::point().with_message(format!("delete_points failed: {}", e))
            })?;

        tracing::debug!(
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
        QdrantConfig::new("http://localhost:6334", "test-key")
    }

    #[test]
    fn test_client_creation() {
        let config = create_test_config();
        let _ = QdrantClient::new(config);
    }

    #[test]
    fn test_config_validation() {
        assert!(
            QdrantConfig::new("http://localhost:6334", "key")
                .validate()
                .is_ok()
        );
        assert!(
            QdrantConfig::new("https://example.com:6334", "key")
                .validate()
                .is_ok()
        );
        assert!(QdrantConfig::new("", "key").validate().is_err());
        assert!(QdrantConfig::new("invalid-url", "key").validate().is_err());
    }
}
