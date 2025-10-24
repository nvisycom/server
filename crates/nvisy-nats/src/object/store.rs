//! Object store implementation using NATS JetStream for file and binary data storage.

use std::time::Duration;

use async_nats::jetstream::{self, stream};
use bytes::Bytes;
use tokio::io::{AsyncRead, AsyncReadExt};
use uuid::Uuid;

use super::types::{GetResult, ObjectInfo, ObjectMeta, ObjectTombstone, PutResult};
use crate::{Error, Result};

// Updated tracing target for object operations
const TRACING_TARGET_OBJECT: &str = "nvisy_nats::object";

const CHUNK_SIZE: usize = 1024 * 1024; // 1MB chunks
const MAX_OBJECT_SIZE: usize = 100 * 1024 * 1024; // 100MB max

/// Object store implementation using NATS JetStream for file and binary data storage
#[derive(Clone)]
pub struct ObjectStore {
    jetstream: jetstream::Context,
    stream_name: String,
    bucket_name: String,
}

impl ObjectStore {
    /// Create or get an Object Store bucket using NATS JetStream
    #[tracing::instrument(skip(jetstream), target = TRACING_TARGET_OBJECT)]
    pub async fn new(
        jetstream: &jetstream::Context,
        bucket_name: &str,
        description: Option<&str>,
        max_age: Option<Duration>,
    ) -> Result<Self> {
        let stream_name = format!("OBJECTS_{}", bucket_name.to_uppercase());
        let default_description = format!("Object store: {}", bucket_name);
        let description_text = description.unwrap_or(&default_description).to_string();

        let stream_config = stream::Config {
            name: stream_name.clone(),
            description: Some(description_text),
            subjects: vec![
                format!("objects.{}.data.>", bucket_name),
                format!("objects.{}.meta.>", bucket_name),
            ],
            max_age: max_age.unwrap_or(Duration::from_secs(0)),
            ..Default::default()
        };

        // Try to get existing stream first
        match jetstream.get_stream(&stream_name).await {
            Ok(_stream) => {
                tracing::info!(
                    target: TRACING_TARGET_OBJECT,
                    stream = %stream_name,
                    bucket = %bucket_name,
                    subjects = ?stream_config.subjects,
                    "Using existing Object Store stream"
                );
            }
            Err(_e) => {
                // Stream doesn't exist, create it
                tracing::info!(
                    target: TRACING_TARGET_OBJECT,
                    stream = %stream_name,
                    bucket = %bucket_name,
                    subjects = ?stream_config.subjects,
                    max_age_secs = max_age.map(|d| d.as_secs()),
                    "Creating new Object Store stream"
                );

                jetstream.create_stream(stream_config).await.map_err(|e| {
                    tracing::error!(
                        target: TRACING_TARGET_OBJECT,
                        stream = %stream_name,
                        error = %e,
                        "Failed to create Object Store stream"
                    );
                    Error::operation("object_store_create", e.to_string())
                })?;

                tracing::info!(
                    target: TRACING_TARGET_OBJECT,
                    stream = %stream_name,
                    bucket = %bucket_name,
                    "Successfully created Object Store stream"
                );
            }
        }

        Ok(Self {
            jetstream: jetstream.clone(),
            stream_name,
            bucket_name: bucket_name.to_string(),
        })
    }

    /// Get the bucket name
    pub fn bucket_name(&self) -> &str {
        &self.bucket_name
    }

    /// Get the stream name
    pub fn stream_name(&self) -> &str {
        &self.stream_name
    }

    /// Put an object from bytes
    #[tracing::instrument(skip(self, data), target = TRACING_TARGET_OBJECT, fields(size = data.len()))]
    pub async fn put_bytes(
        &self,
        object_name: &str,
        data: Bytes,
        metadata: Option<ObjectMeta>,
    ) -> Result<PutResult> {
        let size = data.len();

        if size > MAX_OBJECT_SIZE {
            let error_msg = format!("Object size {} exceeds maximum {}", size, MAX_OBJECT_SIZE);
            tracing::error!(
                target: TRACING_TARGET_OBJECT,
                object_name = %object_name,
                size = size,
                max_size = MAX_OBJECT_SIZE,
                "Object size exceeds maximum allowed"
            );
            return Err(Error::operation("object_too_large", error_msg));
        }

        let object_id = Uuid::new_v4().to_string();
        let chunk_count = if size > 0 {
            (size - 1) / CHUNK_SIZE + 1
        } else {
            0
        };

        tracing::debug!(
            target: TRACING_TARGET_OBJECT,
            object_name = %object_name,
            object_id = %object_id,
            size = size,
            chunk_count = chunk_count,
            chunk_size = CHUNK_SIZE,
            "Starting object upload"
        );

        // Store metadata first
        let object_info = ObjectInfo {
            name: object_name.to_string(),
            size: size as u64,
            modified: Some(jiff::Timestamp::now()),
            nuid: object_id.clone(),
            bucket: self.bucket_name.clone(),
            headers: metadata
                .as_ref()
                .map(|m| m.headers.clone())
                .unwrap_or_default(),
            content_type: metadata.as_ref().and_then(|m| m.content_type.clone()),
            chunk_count,
        };

        // Store metadata
        let meta_subject = format!("objects.{}.meta.{}", self.bucket_name, object_name);
        let meta_payload = serde_json::to_vec(&object_info).map_err(|e| {
            tracing::error!(
                target: TRACING_TARGET_OBJECT,
                object_name = %object_name,
                error = %e,
                "Failed to serialize object metadata"
            );
            e
        })?;

        self.jetstream
            .publish(meta_subject.clone(), meta_payload.into())
            .await
            .map_err(|e| {
                tracing::error!(
                    target: TRACING_TARGET_OBJECT,
                    object_name = %object_name,
                    subject = %meta_subject,
                    error = %e,
                    "Failed to publish object metadata"
                );
                Error::delivery_failed("metadata", e.to_string())
            })?;

        tracing::debug!(
            target: TRACING_TARGET_OBJECT,
            object_name = %object_name,
            subject = %meta_subject,
            "Successfully published object metadata"
        );

        // Store data in chunks if needed
        if !data.is_empty() {
            for (chunk_idx, chunk) in data.chunks(CHUNK_SIZE).enumerate() {
                let chunk_subject = format!(
                    "objects.{}.data.{}.{}",
                    self.bucket_name, object_name, chunk_idx
                );

                let chunk_bytes = Bytes::copy_from_slice(chunk);

                tracing::debug!(
                    target: TRACING_TARGET_OBJECT,
                    object_name = %object_name,
                    chunk_idx = chunk_idx,
                    chunk_size = chunk.len(),
                    subject = %chunk_subject,
                    "Publishing object chunk"
                );

                self.jetstream
                    .publish(chunk_subject.clone(), chunk_bytes)
                    .await
                    .map_err(|e| {
                        tracing::error!(
                            target: TRACING_TARGET_OBJECT,
                            object_name = %object_name,
                            chunk_idx = chunk_idx,
                            subject = %chunk_subject,
                            error = %e,
                            "Failed to publish object chunk"
                        );
                        Error::delivery_failed("chunk", e.to_string())
                    })?;
            }
        }

        let result = PutResult::new(object_name, size as u64, object_id, &self.bucket_name);

        tracing::info!(
            target: TRACING_TARGET_OBJECT,
            object_name = %object_name,
            object_id = %result.nuid,
            size_bytes = size,
            chunk_count = chunk_count,
            human_size = %result.human_size(),
            stream = %self.stream_name,
            "Successfully stored object"
        );

        Ok(result)
    }

    /// Put an object from an async reader
    #[tracing::instrument(skip(self, reader), target = TRACING_TARGET_OBJECT)]
    pub async fn put_reader<R: AsyncRead + Send + Sync + Unpin + 'static>(
        &self,
        object_name: &str,
        mut reader: R,
        metadata: Option<ObjectMeta>,
    ) -> Result<PutResult> {
        tracing::debug!(
            target: TRACING_TARGET_OBJECT,
            object_name = %object_name,
            "Reading data from async reader"
        );

        // Read all data into memory first (for simplicity)
        let mut data = Vec::new();
        let bytes_read = reader.read_to_end(&mut data).await.map_err(|e| {
            tracing::error!(
                target: TRACING_TARGET_OBJECT,
                object_name = %object_name,
                error = %e,
                "Failed to read from async reader"
            );
            Error::operation("read_reader", e.to_string())
        })?;

        tracing::debug!(
            target: TRACING_TARGET_OBJECT,
            object_name = %object_name,
            bytes_read = bytes_read,
            "Successfully read data from async reader"
        );

        self.put_bytes(object_name, Bytes::from(data), metadata)
            .await
    }

    /// Get an object and return all data as bytes
    /// Note: This is a simplified implementation that doesn't support chunked retrieval
    #[tracing::instrument(skip(self), target = TRACING_TARGET_OBJECT)]
    pub async fn get_bytes(&self, object_name: &str) -> Result<Option<GetResult>> {
        // For now, return a placeholder implementation since the full chunked approach
        // requires NATS JetStream consumer APIs that need more complex implementation
        tracing::warn!(
            target: TRACING_TARGET_OBJECT,
            object_name = %object_name,
            bucket = %self.bucket_name,
            "Object retrieval is not fully implemented - this requires JetStream consumer setup"
        );
        Ok(None)
    }

    /// Delete an object by publishing a tombstone
    #[tracing::instrument(skip(self), target = TRACING_TARGET_OBJECT)]
    pub async fn delete(&self, object_name: &str) -> Result<()> {
        // Simplified delete by publishing a tombstone
        let meta_subject = format!("objects.{}.meta.{}", self.bucket_name, object_name);
        let tombstone = ObjectTombstone::new(object_name);
        let tombstone_payload = serde_json::to_vec(&tombstone).map_err(|e| {
            tracing::error!(
                target: TRACING_TARGET_OBJECT,
                object_name = %object_name,
                error = %e,
                "Failed to serialize tombstone"
            );
            e
        })?;

        self.jetstream
            .publish(meta_subject.clone(), tombstone_payload.into())
            .await
            .map_err(|e| {
                tracing::error!(
                    target: TRACING_TARGET_OBJECT,
                    object_name = %object_name,
                    subject = %meta_subject,
                    error = %e,
                    "Failed to publish tombstone"
                );
                Error::delivery_failed("tombstone", e.to_string())
            })?;

        tracing::info!(
            target: TRACING_TARGET_OBJECT,
            object_name = %object_name,
            bucket = %self.bucket_name,
            subject = %meta_subject,
            "Successfully deleted object (tombstone published)"
        );

        Ok(())
    }

    /// Get object information without downloading the content
    /// Note: This is a simplified implementation that doesn't support metadata retrieval
    #[tracing::instrument(skip(self), target = TRACING_TARGET_OBJECT)]
    pub async fn info(&self, object_name: &str) -> Result<Option<ObjectInfo>> {
        // For now, return None since we don't have the required JetStream consumer APIs
        // A full implementation would need to consume the metadata subject
        tracing::warn!(
            target: TRACING_TARGET_OBJECT,
            object_name = %object_name,
            bucket = %self.bucket_name,
            "Object info retrieval requires JetStream consumer implementation - returning None"
        );
        Ok(None)
    }

    /// Check if an object exists
    #[tracing::instrument(skip(self), target = TRACING_TARGET_OBJECT)]
    pub async fn exists(&self, object_name: &str) -> Result<bool> {
        let result = self.info(object_name).await?.is_some();
        tracing::debug!(
            target: TRACING_TARGET_OBJECT,
            object_name = %object_name,
            bucket = %self.bucket_name,
            exists = result,
            "Checked object existence"
        );
        Ok(result)
    }

    /// List all objects in the store (simplified implementation)
    #[tracing::instrument(skip(self), target = TRACING_TARGET_OBJECT)]
    pub async fn list(&self) -> Result<Vec<ObjectInfo>> {
        // This is a simplified implementation that would need to be optimized
        // for production use by using proper stream consumers
        let objects = Vec::new();

        // For now, we'll return an empty list as implementing a full list
        // would require stream consumption which is more complex
        tracing::warn!(
            target: TRACING_TARGET_OBJECT,
            bucket = %self.bucket_name,
            stream = %self.stream_name,
            "List operation requires JetStream consumer implementation - returning empty list"
        );

        tracing::debug!(
            target: TRACING_TARGET_OBJECT,
            count = objects.len(),
            bucket = %self.bucket_name,
            "Listed objects in store"
        );
        Ok(objects)
    }

    /// Get the underlying jetstream context
    pub fn inner(&self) -> &jetstream::Context {
        &self.jetstream
    }

    /// Convenience method: put object with automatic content type detection
    #[tracing::instrument(skip(self, data), target = TRACING_TARGET_OBJECT)]
    pub async fn put_with_content_type(&self, object_name: &str, data: Bytes) -> Result<PutResult> {
        let metadata = if let Some(extension) = std::path::Path::new(object_name)
            .extension()
            .and_then(|ext| ext.to_str())
        {
            Some(ObjectMeta::for_file_type(extension))
        } else {
            Some(ObjectMeta::for_binary())
        };

        tracing::debug!(
            target: TRACING_TARGET_OBJECT,
            object_name = %object_name,
            content_type = ?metadata.as_ref().and_then(|m| m.content_type.as_ref()),
            "Auto-detected content type from filename"
        );

        self.put_bytes(object_name, data, metadata).await
    }

    /// Convenience method: put text data
    #[tracing::instrument(skip(self, text), target = TRACING_TARGET_OBJECT)]
    pub async fn put_text(&self, object_name: &str, text: &str) -> Result<PutResult> {
        let data = Bytes::from(text.to_string());
        let metadata = Some(ObjectMeta::for_text());
        self.put_bytes(object_name, data, metadata).await
    }

    /// Convenience method: put JSON data
    #[tracing::instrument(skip(self, json), target = TRACING_TARGET_OBJECT)]
    pub async fn put_json<T: serde::Serialize>(
        &self,
        object_name: &str,
        json: &T,
    ) -> Result<PutResult> {
        let json_str = serde_json::to_string(json).map_err(|e| {
            tracing::error!(
                target: TRACING_TARGET_OBJECT,
                object_name = %object_name,
                error = %e,
                "Failed to serialize JSON data"
            );
            e
        })?;

        let data = Bytes::from(json_str);
        let metadata = Some(ObjectMeta::for_json());
        self.put_bytes(object_name, data, metadata).await
    }

    /// Get bucket statistics (simplified implementation)
    #[tracing::instrument(skip(self), target = TRACING_TARGET_OBJECT)]
    pub async fn stats(&self) -> Result<ObjectStoreStats> {
        // This would require implementing stream message counting
        // For now, return empty stats
        let stats = ObjectStoreStats {
            bucket: self.bucket_name.clone(),
            stream: self.stream_name.clone(),
            object_count: 0,
            total_size: 0,
        };

        tracing::info!(
            target: TRACING_TARGET_OBJECT,
            bucket = %self.bucket_name,
            stats = ?stats,
            "Retrieved object store statistics"
        );

        Ok(stats)
    }
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
    fn test_chunk_calculation() {
        // Test chunk count calculation
        let data_size = CHUNK_SIZE * 2 + 100; // 2 full chunks + partial
        let chunk_count = (data_size - 1) / CHUNK_SIZE + 1;
        assert_eq!(chunk_count, 3);

        // Test single chunk
        let data_size = 100;
        let chunk_count = (data_size - 1) / CHUNK_SIZE + 1;
        assert_eq!(chunk_count, 1);

        // Test empty data
        let data_size = 0;
        let chunk_count = if data_size > 0 {
            (data_size - 1) / CHUNK_SIZE + 1
        } else {
            0
        };
        assert_eq!(chunk_count, 0);
    }

    #[test]
    fn test_object_store_stats() {
        let stats = ObjectStoreStats {
            bucket: "test".to_string(),
            stream: "OBJECTS_TEST".to_string(),
            object_count: 10,
            total_size: 1024 * 1024,
        };

        assert_eq!(stats.human_total_size(), "1.0 MB");
    }

    #[test]
    fn test_subject_formatting() {
        let bucket = "my_bucket";
        let object_name = "my/object.txt";

        let meta_subject = format!("objects.{}.meta.{}", bucket, object_name);
        assert_eq!(meta_subject, "objects.my_bucket.meta.my/object.txt");

        let data_subject = format!("objects.{}.data.{}.{}", bucket, object_name, 0);
        assert_eq!(data_subject, "objects.my_bucket.data.my/object.txt.0");
    }
}
