//! Object operations for MinIO storage.
//!
//! This module provides comprehensive object operations including upload, download,
//! deletion, and listing with support for metadata, tags, and streaming.

use std::collections::HashMap;

use bytes::Bytes;
use futures::StreamExt;
use minio::s3::segmented_bytes::SegmentedBytes;
use minio::s3::types::{S3Api, ToStream};
use time::OffsetDateTime;
use tracing::{debug, error, info, instrument, warn};

use crate::types::{ObjectInfo, UploadContext};
use crate::{Error, MinioClient, Result, TRACING_TARGET_OBJECTS};

/// Result of an upload operation.
#[derive(Debug, Clone)]
pub struct UploadResult {
    /// Object key/path that was uploaded.
    pub key: String,
    /// Size of the uploaded object in bytes.
    pub size: u64,
    /// ETag of the uploaded object.
    pub etag: String,
    /// Upload duration.
    pub duration: std::time::Duration,
}

/// Result of a download operation.
#[derive(Debug, Clone)]
pub struct DownloadResult {
    /// Object key/path that was downloaded.
    pub key: String,
    /// Size of the downloaded object in bytes.
    pub size: u64,
    /// Content type of the downloaded object.
    pub content_type: Option<String>,
    /// Download duration.
    pub duration: std::time::Duration,
    /// Object metadata.
    pub metadata: HashMap<String, String>,
}

/// Result of a list objects operation.
#[derive(Debug, Clone)]
pub struct ListObjectsResult {
    /// List of objects.
    pub objects: Vec<ObjectInfo>,
    /// Continuation token for pagination.
    pub next_continuation_token: Option<String>,
    /// Whether the result is truncated.
    pub is_truncated: bool,
}

/// Object operations with a required MinIO client.
#[derive(Debug, Clone)]
pub struct ObjectOperations {
    client: MinioClient,
}

impl ObjectOperations {
    /// Creates new ObjectOperations with a MinIO client.
    pub fn new(client: MinioClient) -> Self {
        Self { client }
    }

    /// Uploads a file to MinIO storage.
    ///
    /// # Arguments
    ///
    /// * `bucket` - Name of the bucket
    /// * `key` - Object key/path
    /// * `data` - File data as bytes
    /// * `metadata` - Optional metadata
    /// * `tags` - Optional tags
    ///
    /// # Errors
    ///
    /// Returns an error if the upload fails.
    #[instrument(skip(self, data), target = TRACING_TARGET_OBJECTS, fields(bucket = %bucket, key = %key))]
    pub async fn upload_file<T: AsRef<[u8]> + Send>(
        &self,
        bucket: &str,
        key: &str,
        data: T,
        metadata: Option<crate::types::ObjectMetadata>,
        tags: Option<crate::types::ObjectTags>,
    ) -> Result<UploadResult> {
        let data_ref = data.as_ref();
        let size = data_ref.len() as u64;

        debug!(
            target: TRACING_TARGET_OBJECTS,
            bucket = %bucket,
            key = %key,
            size = %size,
            "Uploading file"
        );

        // Execute upload hooks if any
        if metadata.is_some() {
            let _upload_ctx = UploadContext::new(bucket.to_string(), key.to_string(), size);
            // Hook execution would go here
        }

        let start = std::time::Instant::now();

        // Convert data to SegmentedBytes for MinIO SDK
        let bytes_data = Bytes::copy_from_slice(data_ref);
        let segmented_data = SegmentedBytes::from(bytes_data);

        // Use put_object directly - MinIO SDK handles the upload
        let result = self
            .client
            .as_inner()
            .put_object(bucket, key, segmented_data)
            .send()
            .await
            .map_err(Error::Client);

        let elapsed = start.elapsed();

        match result {
            Ok(response) => {
                let etag = response.etag;

                info!(
                    target: TRACING_TARGET_OBJECTS,
                    bucket = %bucket,
                    key = %key,
                    size = %size,
                    etag = %etag,
                    elapsed = ?elapsed,
                    "File uploaded successfully"
                );

                Ok(UploadResult {
                    key: key.to_string(),
                    size,
                    etag,
                    duration: elapsed,
                })
            }
            Err(e) => {
                error!(
                    target: TRACING_TARGET_OBJECTS,
                    bucket = %bucket,
                    key = %key,
                    error = %e,
                    elapsed = ?elapsed,
                    "Failed to upload file"
                );
                Err(e)
            }
        }
    }

    /// Downloads a file from MinIO storage.
    ///
    /// # Arguments
    ///
    /// * `bucket` - Name of the bucket
    /// * `key` - Object key/path
    ///
    /// # Errors
    ///
    /// Returns an error if the download fails or object doesn't exist.
    #[instrument(skip(self), target = TRACING_TARGET_OBJECTS, fields(bucket = %bucket, key = %key))]
    pub async fn download_file(&self, bucket: &str, key: &str) -> Result<(Bytes, DownloadResult)> {
        debug!(
            target: TRACING_TARGET_OBJECTS,
            bucket = %bucket,
            key = %key,
            "Downloading file"
        );

        let start = std::time::Instant::now();

        let result = self
            .client
            .as_inner()
            .get_object(bucket, key)
            .send()
            .await
            .map_err(Error::Client);

        match result {
            Ok(response) => {
                // Extract headers before consuming response
                let content_type = response
                    .headers
                    .get("content-type")
                    .and_then(|v| v.to_str().ok())
                    .map(String::from);

                // Extract metadata from headers
                let metadata = response
                    .headers
                    .iter()
                    .filter_map(|(k, v)| {
                        if k.as_str().starts_with("x-amz-meta-") {
                            let key = k.as_str().strip_prefix("x-amz-meta-")?.to_string();
                            let value = v.to_str().ok()?.to_string();
                            Some((key, value))
                        } else {
                            None
                        }
                    })
                    .collect::<HashMap<String, String>>();

                // Read the body - convert ObjectContent to SegmentedBytes then to Bytes
                let segmented = response
                    .content
                    .to_segmented_bytes()
                    .await
                    .map_err(|e| Error::Io(e))?;
                let data = segmented.to_bytes();

                let size = data.len() as u64;
                let elapsed = start.elapsed();

                info!(
                    target: TRACING_TARGET_OBJECTS,
                    bucket = %bucket,
                    key = %key,
                    size = %size,
                    elapsed = ?elapsed,
                    "File downloaded successfully"
                );

                Ok((
                    data,
                    DownloadResult {
                        key: key.to_string(),
                        size,
                        content_type,
                        duration: elapsed,
                        metadata,
                    },
                ))
            }
            Err(e) => {
                let elapsed = start.elapsed();
                error!(
                    target: TRACING_TARGET_OBJECTS,
                    bucket = %bucket,
                    key = %key,
                    error = %e,
                    elapsed = ?elapsed,
                    "Failed to download file"
                );
                Err(e)
            }
        }
    }

    /// Lists objects in a bucket with optional prefix filtering.
    ///
    /// # Arguments
    ///
    /// * `bucket` - Name of the bucket
    /// * `prefix` - Optional prefix to filter objects
    /// * `recursive` - Whether to list objects recursively
    /// * `max_keys` - Maximum number of keys to return (default: 1000)
    ///
    /// # Errors
    ///
    /// Returns an error if the listing fails.
    #[instrument(skip(self), target = TRACING_TARGET_OBJECTS, fields(bucket = %bucket))]
    pub async fn list_objects(
        &self,
        bucket: &str,
        prefix: Option<&str>,
        recursive: bool,
        max_keys: Option<i32>,
    ) -> Result<ListObjectsResult> {
        debug!(
            target: TRACING_TARGET_OBJECTS,
            bucket = %bucket,
            prefix = ?prefix,
            recursive = %recursive,
            "Listing objects"
        );

        let start = std::time::Instant::now();

        // Build the list request
        let mut list_request = self.client.as_inner().list_objects(bucket);

        if let Some(p) = prefix {
            list_request = list_request.prefix(Some(p.to_string()));
        }

        if !recursive {
            list_request = list_request.delimiter(Some("/".to_string()));
        }

        if let Some(max) = max_keys {
            list_request = list_request.max_keys(Some(max as u16));
        }

        // Use to_stream to get the stream, then get first page
        let mut stream = list_request.to_stream().await;

        let result = stream.next().await;

        let elapsed = start.elapsed();

        match result {
            Some(Ok(response)) => {
                let objects: Vec<ObjectInfo> = response
                    .contents
                    .into_iter()
                    .filter_map(|obj| {
                        let size = obj.size.unwrap_or(0) as u64;

                        // Parse last modified time
                        let last_modified = obj
                            .last_modified
                            .and_then(|dt| {
                                // Convert chrono DateTime to time::OffsetDateTime
                                time::OffsetDateTime::from_unix_timestamp(dt.timestamp()).ok()
                            })
                            .unwrap_or_else(OffsetDateTime::now_utc);

                        let mut object_info =
                            ObjectInfo::new(obj.name.clone(), size, last_modified);

                        // Add ETag if present
                        if let Some(etag) = obj.etag {
                            object_info = object_info.with_etag(etag);
                        }

                        Some(object_info)
                    })
                    .collect();

                let is_truncated = response.is_truncated;
                let next_continuation_token = response.next_continuation_token;

                info!(
                    target: TRACING_TARGET_OBJECTS,
                    bucket = %bucket,
                    count = objects.len(),
                    is_truncated = %is_truncated,
                    elapsed = ?elapsed,
                    "Objects listed successfully"
                );

                Ok(ListObjectsResult {
                    objects,
                    next_continuation_token,
                    is_truncated,
                })
            }
            Some(Err(e)) => {
                error!(
                    target: TRACING_TARGET_OBJECTS,
                    bucket = %bucket,
                    error = %e,
                    elapsed = ?elapsed,
                    "Failed to list objects"
                );
                Err(Error::Client(e))
            }
            None => {
                // Empty stream means no objects
                info!(
                    target: TRACING_TARGET_OBJECTS,
                    bucket = %bucket,
                    count = 0,
                    elapsed = ?elapsed,
                    "No objects found"
                );
                Ok(ListObjectsResult {
                    objects: Vec::new(),
                    next_continuation_token: None,
                    is_truncated: false,
                })
            }
        }
    }

    /// Deletes an object from MinIO storage.
    ///
    /// # Arguments
    ///
    /// * `bucket` - Name of the bucket
    /// * `key` - Object key/path to delete
    ///
    /// # Errors
    ///
    /// Returns an error if the deletion fails.
    #[instrument(skip(self), target = TRACING_TARGET_OBJECTS, fields(bucket = %bucket, key = %key))]
    pub async fn delete_object(&self, bucket: &str, key: &str) -> Result<()> {
        debug!(
            target: TRACING_TARGET_OBJECTS,
            bucket = %bucket,
            key = %key,
            "Deleting object"
        );

        let start = std::time::Instant::now();

        let result = self
            .client
            .as_inner()
            .delete_object(bucket, key)
            .send()
            .await
            .map_err(Error::Client);

        let elapsed = start.elapsed();

        match result {
            Ok(_) => {
                info!(
                    target: TRACING_TARGET_OBJECTS,
                    bucket = %bucket,
                    key = %key,
                    elapsed = ?elapsed,
                    "Object deleted successfully"
                );
                Ok(())
            }
            Err(e) => {
                error!(
                    target: TRACING_TARGET_OBJECTS,
                    bucket = %bucket,
                    key = %key,
                    error = %e,
                    elapsed = ?elapsed,
                    "Failed to delete object"
                );
                Err(e)
            }
        }
    }

    /// Gets metadata and information about an object without downloading it.
    ///
    /// # Arguments
    ///
    /// * `bucket` - Name of the bucket
    /// * `key` - Object key/path
    ///
    /// # Errors
    ///
    /// Returns an error if the object doesn't exist or stat fails.
    #[instrument(skip(self), target = TRACING_TARGET_OBJECTS, fields(bucket = %bucket, key = %key))]
    pub async fn get_object_info(&self, bucket: &str, key: &str) -> Result<ObjectInfo> {
        debug!(
            target: TRACING_TARGET_OBJECTS,
            bucket = %bucket,
            key = %key,
            "Getting object info"
        );

        let start = std::time::Instant::now();

        let result = self
            .client
            .as_inner()
            .stat_object(bucket, key)
            .send()
            .await
            .map_err(Error::Client);

        let elapsed = start.elapsed();

        match result {
            Ok(response) => {
                let size = response.size as u64;

                // Parse last modified time
                let last_modified = response
                    .last_modified
                    .and_then(|dt| time::OffsetDateTime::from_unix_timestamp(dt.timestamp()).ok())
                    .unwrap_or_else(OffsetDateTime::now_utc);

                let mut object_info = ObjectInfo::new(key, size, last_modified);

                // Add ETag
                object_info = object_info.with_etag(response.etag);

                // Extract content type from headers
                if let Some(content_type) = response
                    .headers
                    .get("content-type")
                    .and_then(|v| v.to_str().ok())
                {
                    object_info = object_info.with_content_type(content_type);
                }

                // Extract user metadata
                let user_metadata: HashMap<String, String> = response
                    .headers
                    .iter()
                    .filter_map(|(k, v)| {
                        if k.as_str().starts_with("x-amz-meta-") {
                            let key = k.as_str().strip_prefix("x-amz-meta-")?.to_string();
                            let value = v.to_str().ok()?.to_string();
                            Some((key, value))
                        } else {
                            None
                        }
                    })
                    .collect();

                object_info = object_info.with_metadata(user_metadata);

                debug!(
                    target: TRACING_TARGET_OBJECTS,
                    bucket = %bucket,
                    key = %key,
                    size = %size,
                    elapsed = ?elapsed,
                    "Object info retrieved successfully"
                );

                Ok(object_info)
            }
            Err(e) => {
                error!(
                    target: TRACING_TARGET_OBJECTS,
                    bucket = %bucket,
                    key = %key,
                    error = %e,
                    elapsed = ?elapsed,
                    "Failed to get object info"
                );
                Err(e)
            }
        }
    }

    /// Deletes multiple objects from MinIO storage in a single request.
    ///
    /// # Arguments
    ///
    /// * `bucket` - Name of the bucket
    /// * `keys` - List of object keys to delete
    ///
    /// # Errors
    ///
    /// Returns an error if the batch deletion fails.
    #[instrument(skip(self, keys), target = TRACING_TARGET_OBJECTS, fields(bucket = %bucket, count = keys.len()))]
    pub async fn delete_objects(&self, bucket: &str, keys: Vec<String>) -> Result<()> {
        if keys.is_empty() {
            warn!(
                target: TRACING_TARGET_OBJECTS,
                bucket = %bucket,
                "No keys provided for batch deletion"
            );
            return Ok(());
        }

        let count = keys.len();

        debug!(
            target: TRACING_TARGET_OBJECTS,
            bucket = %bucket,
            count = %count,
            "Deleting multiple objects"
        );

        let start = std::time::Instant::now();

        // MinIO delete_objects expects Vec<ObjectToDelete>
        use minio::s3::builders::ObjectToDelete;
        let objects_to_delete: Vec<ObjectToDelete> = keys
            .into_iter()
            .map(|key| ObjectToDelete::from(key.as_str()))
            .collect();

        let result = self
            .client
            .as_inner()
            .delete_objects::<&str, ObjectToDelete>(bucket, objects_to_delete)
            .send()
            .await
            .map_err(Error::Client);

        let elapsed = start.elapsed();

        match result {
            Ok(_) => {
                info!(
                    target: TRACING_TARGET_OBJECTS,
                    bucket = %bucket,
                    count = %count,
                    elapsed = ?elapsed,
                    "Objects deleted successfully"
                );
                Ok(())
            }
            Err(e) => {
                error!(
                    target: TRACING_TARGET_OBJECTS,
                    bucket = %bucket,
                    count = %count,
                    error = %e,
                    elapsed = ?elapsed,
                    "Failed to delete objects"
                );
                Err(e)
            }
        }
    }
}
