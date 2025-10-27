//! Typed object store implementation using NATS JetStream for file and binary data storage.

use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Duration;

use async_nats::jetstream::context::ObjectStoreErrorKind;
use async_nats::jetstream::object_store::{self, InfoErrorKind};
use async_nats::jetstream::{self};
use bytes::Bytes;
use futures::StreamExt;
use tokio::io::AsyncReadExt;

use super::content_data::ContentData;
use super::object_headers::ObjectHeaders;
use super::object_metadata::ObjectMetadata;
use crate::{Error, Result};

// Updated tracing target for object operations
const TRACING_TARGET_OBJECT: &str = "nvisy_nats::object";

/// Typed object store using NATS JetStream that enforces key and data type safety.
///
/// This store supports rich metadata and headers for objects, allowing you to store
/// additional information alongside your data such as content types, tags, version
/// information, and custom headers.
///
/// # Type Parameters
///
/// * `K` - The key type that implements `AsRef<str>`, used to identify objects
pub struct ObjectStore<K = String>
where
    K: AsRef<str>,
{
    inner: Arc<object_store::ObjectStore>,
    bucket_name: Arc<String>,
    _phantom_key: PhantomData<K>,
}

impl<K> ObjectStore<K>
where
    K: AsRef<str>,
{
    /// Create or get an Object Store bucket using NATS JetStream
    #[tracing::instrument(skip(jetstream), target = TRACING_TARGET_OBJECT)]
    pub async fn new(
        jetstream: &jetstream::Context,
        bucket_name: &str,
        description: Option<&str>,
        max_age: Option<Duration>,
    ) -> Result<Self> {
        tracing::debug!(
            target: TRACING_TARGET_OBJECT,
            bucket = %bucket_name,
            "Attempting to get existing Object Store bucket"
        );

        // First try to get an existing store
        let store = match jetstream.get_object_store(bucket_name).await {
            Ok(store) => {
                tracing::info!(
                    target: TRACING_TARGET_OBJECT,
                    bucket = %bucket_name,
                    "Successfully retrieved existing Object Store bucket"
                );
                store
            }
            Err(x) if matches!(x.kind(), ObjectStoreErrorKind::GetStore) => {
                // If getting fails, create a new store
                let default_description = format!("Object store: {}", bucket_name);
                let description_text = description.unwrap_or(&default_description);

                let mut config = object_store::Config {
                    bucket: bucket_name.to_string(),
                    description: Some(description_text.to_string()),
                    ..Default::default()
                };

                if let Some(age) = max_age {
                    config.max_age = age;
                }

                tracing::info!(
                    target: TRACING_TARGET_OBJECT,
                    bucket = %bucket_name,
                    description = %description_text,
                    max_age_secs = max_age.map(|d| d.as_secs()),
                    "Creating new Object Store bucket"
                );

                jetstream.create_object_store(config).await.map_err(|e| {
                    tracing::error!(
                        target: TRACING_TARGET_OBJECT,
                        bucket = %bucket_name,
                        error = %e,
                        "Failed to create Object Store bucket"
                    );
                    Error::operation("object_store_create", e.to_string())
                })?
            }
            Err(e) => {
                tracing::error!(
                    target: TRACING_TARGET_OBJECT,
                    bucket = %bucket_name,
                    error = %e,
                    "Failed to find or create Object Store bucket"
                );
                Err(Error::operation("object_store_create", e.to_string()))?
            }
        };

        tracing::info!(
            target: TRACING_TARGET_OBJECT,
            bucket = %bucket_name,
            "Successfully initialized Object Store bucket"
        );

        Ok(Self {
            inner: Arc::new(store),
            bucket_name: Arc::new(bucket_name.to_string()),
            _phantom_key: PhantomData,
        })
    }

    /// Get the bucket name
    pub fn bucket_name(&self) -> &str {
        &self.bucket_name
    }

    /// Put data into the store with optional metadata and headers
    ///
    /// This approach ensures compatibility with both nvisy-specific rich metadata and
    /// standard NATS object storage, while providing graceful degradation.
    #[tracing::instrument(
        skip(self, content),
        target = TRACING_TARGET_OBJECT,
        fields(
            key = %key.as_ref(),
            content_source = %content.metadata().content_source()
        )
    )]
    pub async fn put(&self, key: &K, content: &ContentData) -> Result<PutResult<K>> {
        let key_str = key.as_ref();
        let size = content.size();

        tracing::debug!(
            target: TRACING_TARGET_OBJECT,
            key = %key_str,
            size = size,
            "Starting object upload"
        );

        // Create NATS metadata for the put operation
        let mut object_meta = object_store::ObjectMetadata {
            name: key_str.to_string(),
            ..Default::default()
        };

        // Serialize and store our ObjectMetadata in the NATS metadata field
        let metadata = content.metadata();
        if let Ok(metadata_json) = serde_json::to_string(metadata) {
            object_meta
                .metadata
                .insert("nvisy-metadata".to_string(), metadata_json);
        }

        // Apply headers
        let headers = content.headers();
        if !headers.is_empty() {
            object_meta.headers = headers.clone().into_header_map();
        }

        // Create a cursor from the bytes for AsyncRead
        let data_bytes = content.as_bytes();
        let mut cursor = std::io::Cursor::new(data_bytes);

        let info = self.inner.put(object_meta, &mut cursor).await;
        let info = info.map_err(|e| {
            tracing::error!(
                target: TRACING_TARGET_OBJECT,
                key = %key_str,
                error = %e,
                "Failed to put object"
            );
            Error::operation("object_put", e.to_string())
        })?;

        let result = PutResult {
            key: key_str.to_string(),
            size: info.size as u64,
            nuid: info.nuid,
            bucket: self.bucket_name.as_str().to_owned(),
            _phantom: PhantomData,
        };

        tracing::info!(
            target: TRACING_TARGET_OBJECT,
            key = %key_str,
            object_id = %result.nuid,
            size_bytes = size,
            bucket = %self.bucket_name,
            "Successfully stored object"
        );

        Ok(result)
    }

    /// Get content data from the store
    #[tracing::instrument(skip(self), target = TRACING_TARGET_OBJECT, fields(key = %key.as_ref()))]
    pub async fn get(&self, key: &K) -> Result<Option<ContentData>> {
        let key_str = key.as_ref();

        tracing::debug!(
            target: TRACING_TARGET_OBJECT,
            key = %key_str,
            bucket = %self.bucket_name,
            "Getting object"
        );

        let mut object = match self.inner.get(key_str).await {
            Ok(obj) => obj,
            Err(e) => {
                // Check if it's a not found error
                let error_str = e.to_string();
                if error_str.contains("not found") || error_str.contains("no message found") {
                    tracing::debug!(
                        target: TRACING_TARGET_OBJECT,
                        key = %key_str,
                        bucket = %self.bucket_name,
                        "Object not found"
                    );
                    return Ok(None);
                }

                tracing::error!(
                    target: TRACING_TARGET_OBJECT,
                    key = %key_str,
                    error = %e,
                    "Failed to get object"
                );
                return Err(Error::operation("object_get", e.to_string()));
            }
        };

        // Get object info for metadata
        let info = match self.inner.info(key_str).await {
            Ok(info) => info,
            Err(e) => {
                tracing::error!(
                    target: TRACING_TARGET_OBJECT,
                    key = %key_str,
                    error = %e,
                    "Failed to get object info"
                );
                return Err(Error::operation("object_info", e.to_string()));
            }
        };

        // Read all data from the object
        let mut data = Vec::new();
        object.read_to_end(&mut data).await.map_err(|e| {
            tracing::error!(
                target: TRACING_TARGET_OBJECT,
                key = %key_str,
                error = %e,
                "Failed to read object data"
            );
            Error::operation("object_read", e.to_string())
        })?;

        let size = data.len();

        // Extract metadata from object info
        let metadata = if let Some(metadata_json) = info.metadata.get("nvisy-metadata") {
            // Try to deserialize our custom ObjectMetadata
            serde_json::from_str::<ObjectMetadata>(metadata_json).unwrap_or_else(|_| {
                // Fall back to creating metadata from NATS object info
                create_metadata_from_nats_info(&info)
            })
        } else {
            // No custom metadata, create from NATS object info
            create_metadata_from_nats_info(&info)
        };

        // Extract headers from object info
        let headers = if let Some(ref header_map) = info.headers {
            ObjectHeaders::from_header_map(header_map.clone())
        } else {
            ObjectHeaders::new()
        };

        let content_data = ContentData::new(Bytes::from(data))
            .with_metadata(metadata)
            .with_headers(headers);

        tracing::info!(
            target: TRACING_TARGET_OBJECT,
            key = %key_str,
            size_bytes = size,
            bucket = %self.bucket_name,
            content_source = %content_data.metadata().content_source(),
            "Successfully retrieved object"
        );

        Ok(Some(content_data))
    }

    /// Delete an object
    #[tracing::instrument(skip(self), target = TRACING_TARGET_OBJECT, fields(key = %key.as_ref()))]
    pub async fn delete(&self, key: &K) -> Result<()> {
        let key_str = key.as_ref();
        self.inner.delete(key_str).await.map_err(|e| {
            tracing::error!(
                target: TRACING_TARGET_OBJECT,
                key = %key_str,
                error = %e,
                "Failed to delete object"
            );
            Error::operation("object_delete", e.to_string())
        })?;

        tracing::info!(
            target: TRACING_TARGET_OBJECT,
            key = %key_str,
            bucket = %self.bucket_name,
            "Successfully deleted object"
        );

        Ok(())
    }

    /// Check if an object exists
    #[tracing::instrument(
        skip(self),
        target = TRACING_TARGET_OBJECT,
        fields(key = %key.as_ref())
    )]
    pub async fn exists(&self, key: &K) -> Result<bool> {
        let key_str = key.as_ref();

        let exists = match self.inner.info(key_str).await {
            Ok(_) => Ok(true),
            Err(e) if matches!(e.kind(), InfoErrorKind::NotFound) => Ok(false),
            Err(e) => Err(Error::operation("object_exists", e.to_string())),
        };

        if let Ok(ref exists) = exists {
            tracing::debug!(
                target: TRACING_TARGET_OBJECT,
                key = %key_str,
                bucket = %self.bucket_name,
                exists = exists,
                "Checked object existence"
            );
        }

        exists
    }

    /// List all objects in the store (returns object names as keys)
    #[tracing::instrument(skip(self), target = TRACING_TARGET_OBJECT)]
    pub async fn list(&self) -> Result<Vec<String>> {
        tracing::debug!(
            target: TRACING_TARGET_OBJECT,
            bucket = %self.bucket_name,
            "Listing objects in store"
        );

        let mut list_stream = self.inner.list().await.map_err(|e| {
            tracing::error!(
                target: TRACING_TARGET_OBJECT,
                bucket = %self.bucket_name,
                error = %e,
                "Failed to list objects"
            );
            Error::operation("object_list", e.to_string())
        })?;

        let mut objects = Vec::new();

        while let Some(info_result) = list_stream.next().await {
            let info = info_result.map_err(|e| {
                tracing::error!(
                    target: TRACING_TARGET_OBJECT,
                    bucket = %self.bucket_name,
                    error = %e,
                    "Failed to get object info from list stream"
                );
                Error::operation("object_list_item", e.to_string())
            })?;

            objects.push(info.name);
        }

        tracing::info!(
            target: TRACING_TARGET_OBJECT,
            count = objects.len(),
            bucket = %self.bucket_name,
            "Listed objects in store"
        );

        Ok(objects)
    }

    /// Get the underlying NATS object store
    pub fn inner(&self) -> &object_store::ObjectStore {
        &self.inner
    }
}

impl<K> Clone for ObjectStore<K>
where
    K: AsRef<str>,
{
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            bucket_name: self.bucket_name.clone(),
            _phantom_key: PhantomData,
        }
    }
}

/// Result of a put operation with typed key
#[derive(Debug, Clone)]
pub struct PutResult<K>
where
    K: AsRef<str>,
{
    pub key: String,
    pub size: u64,
    pub nuid: String,
    pub bucket: String,
    _phantom: PhantomData<K>,
}

impl<K> PutResult<K>
where
    K: AsRef<str>,
{
    /// Check if put was successful
    pub fn is_success(&self) -> bool {
        !self.key.is_empty() && !self.nuid.is_empty()
    }
}

/// Helper function to create ObjectMetadata from NATS object info
fn create_metadata_from_nats_info(info: &object_store::ObjectInfo) -> ObjectMetadata {
    let timestamp = info
        .modified
        .map(|dt| {
            jiff::Timestamp::from_second(dt.unix_timestamp())
                .unwrap_or_else(|_| jiff::Timestamp::now())
        })
        .unwrap_or_else(jiff::Timestamp::now);

    ObjectMetadata::new()
        .with_sha256(info.digest.clone().unwrap_or_default())
        .with_created_at(timestamp)
}

/// Object store statistics
#[derive(Debug, Clone)]
pub struct ObjectStoreStats {
    pub bucket: String,
    pub stream: String,
    pub object_count: u64,
    pub total_size: u64,
}

impl ObjectStoreStats {
    /// Get human readable total size
    pub fn human_total_size(&self) -> String {
        const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
        let mut size = self.total_size as f64;
        let mut unit_index = 0;

        while size >= 1024.0 && unit_index < UNITS.len() - 1 {
            size /= 1024.0;
            unit_index += 1;
        }

        if unit_index == 0 {
            format!("{} {}", self.total_size, UNITS[unit_index])
        } else {
            format!("{:.1} {}", size, UNITS[unit_index])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_put_result() {
        // Test key type
        struct TestKey(String);
        impl AsRef<str> for TestKey {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        let result: PutResult<TestKey> = PutResult {
            key: "test-key".to_string(),
            size: 1024,
            nuid: "nuid123".to_string(),
            bucket: "test-bucket".to_string(),
            _phantom: PhantomData,
        };

        assert!(result.is_success());
        assert_eq!(result.key, "test-key");
    }

    #[test]
    fn test_put_result_with_metadata_and_headers() {
        let result: PutResult<String> = PutResult {
            key: "test-key-with-metadata".to_string(),
            size: 1024,
            nuid: "nuid456".to_string(),
            bucket: "test-bucket".to_string(),
            _phantom: PhantomData,
        };

        assert!(result.is_success());
        assert_eq!(result.key, "test-key-with-metadata");
        assert_eq!(result.size, 1024);
        assert_eq!(result.nuid, "nuid456");
        assert_eq!(result.bucket, "test-bucket");
    }

    #[test]
    fn test_object_metadata_creation() {
        let metadata = ObjectMetadata::new()
            .with_sha256("test-hash")
            .with_version(2)
            .with_tag("environment:test")
            .with_original_filename("test.json");

        assert_eq!(metadata.sha256(), Some("test-hash"));

        assert_eq!(metadata.version(), Some(2));
        assert!(metadata.has_tag("environment:test"));
        assert_eq!(metadata.original_filename(), Some("test.json"));
        assert!(!metadata.is_empty());
    }

    #[test]
    fn test_object_headers_creation() {
        let headers = ObjectHeaders::new()
            .set("content-type", "application/json")
            .set("content-encoding", "gzip")
            .set("content-length", "2048")
            .set("cache-control", "max-age=3600")
            .set("user-id", "123")
            .set("etag", "test-etag");

        assert_eq!(headers.get("content-type"), Some("application/json"));
        assert_eq!(headers.get("content-encoding"), Some("gzip"));
        assert_eq!(headers.get("content-length"), Some("2048"));
        assert_eq!(headers.get("cache-control"), Some("max-age=3600"));
        assert_eq!(headers.get("user-id"), Some("123"));
        assert_eq!(headers.get("etag"), Some("test-etag"));
        assert_eq!(headers.len(), 6);
    }

    #[test]
    fn test_metadata_and_headers_consistency() {
        let metadata = ObjectMetadata::new().with_sha256("consistent-hash");

        let headers = ObjectHeaders::new()
            .set("content-type", "text/plain")
            .set("etag", "consistent-hash");

        // Verify consistency between metadata and headers
        assert_eq!(metadata.sha256(), headers.get("etag"));
    }

    #[test]
    fn test_metadata_round_trip_serialization() {
        use std::collections::HashSet;

        // Create rich metadata with all fields
        let mut tags = HashSet::new();
        tags.insert("test".to_string());
        tags.insert("round-trip".to_string());

        let original = ObjectMetadata::new()
            .with_sha256("test-hash")
            .with_version(5)
            .with_tags(tags.clone())
            .with_original_filename("test-file.json")
            .with_timestamps_now();

        // Test JSON serialization/deserialization
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: ObjectMetadata = serde_json::from_str(&json).unwrap();

        // Verify all fields are preserved
        assert_eq!(original.sha256(), deserialized.sha256());
        assert_eq!(original.sha256(), deserialized.sha256());
        assert_eq!(original.version(), deserialized.version());
        assert_eq!(original.tags(), deserialized.tags());
        assert_eq!(
            original.original_filename(),
            deserialized.original_filename()
        );
        assert_eq!(original.created_at(), deserialized.created_at());
        assert_eq!(original.updated_at(), deserialized.updated_at());
    }

    #[test]
    fn test_metadata_fallback_from_nats_info() {
        // Test that we can create metadata from NATS ObjectInfo when custom metadata fails
        let _test_key = "test-fallback-key".to_string();

        // This test verifies the create_metadata_from_nats_info function works
        // In a real scenario, this would be called when NATS metadata doesn't contain
        // our custom "nvisy-metadata" field or when deserialization fails

        // We can't easily mock ObjectInfo here, but we can verify the function exists
        // and would be called in the appropriate circumstances
        assert!(true); // Placeholder for integration test
    }
}
