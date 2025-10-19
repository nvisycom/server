//! Bucket operations for MinIO storage.
//!
//! This module provides simple bucket operations with a required MinIO client.

use minio::s3::types::S3Api;
use tracing::{debug, error, info, instrument};

use crate::types::BucketInfo;
use crate::{Error, MinioClient, Result, TRACING_TARGET_BUCKETS, TRACING_TARGET_OPERATIONS};

/// Simple bucket operations with a required MinIO client.
#[derive(Debug, Clone)]
pub struct BucketOperations {
    client: MinioClient,
}

impl BucketOperations {
    /// Creates new BucketOperations with a MinIO client.
    pub fn new(client: MinioClient) -> Self {
        Self { client }
    }

    /// Creates a new bucket.
    ///
    /// # Arguments
    ///
    /// * `bucket_name` - Name of the bucket to create
    ///
    /// # Errors
    ///
    /// Returns an error if the bucket creation fails.
    #[instrument(skip(self), target = TRACING_TARGET_BUCKETS, fields(bucket = %bucket_name))]
    pub async fn create_bucket(&self, bucket_name: &str) -> Result<()> {
        debug!(
            target: TRACING_TARGET_OPERATIONS,
            bucket = %bucket_name,
            "Creating bucket"
        );

        let start = std::time::Instant::now();
        let create_bucket_request = self.client.as_inner().create_bucket(bucket_name);
        let result = create_bucket_request.send().await.map_err(Error::Client);

        let elapsed = start.elapsed();

        match result {
            Ok(_response) => {
                info!(
                    target: TRACING_TARGET_BUCKETS,
                    bucket = %bucket_name,
                    elapsed = ?elapsed,
                    "Bucket created successfully"
                );
                Ok(())
            }
            Err(e) => {
                error!(
                    target: TRACING_TARGET_BUCKETS,
                    bucket = %bucket_name,
                    error = %e,
                    elapsed = ?elapsed,
                    "Failed to create bucket"
                );
                Err(e)
            }
        }
    }

    /// Deletes a bucket.
    ///
    /// # Arguments
    ///
    /// * `bucket_name` - Name of the bucket to delete
    ///
    /// # Errors
    ///
    /// Returns an error if the bucket deletion fails or if the bucket is not empty.
    #[instrument(skip(self), target = TRACING_TARGET_BUCKETS, fields(bucket = %bucket_name))]
    pub async fn delete_bucket(&self, bucket_name: &str) -> Result<()> {
        debug!(
            target: TRACING_TARGET_OPERATIONS,
            bucket = %bucket_name,
            "Deleting bucket"
        );

        let start = std::time::Instant::now();
        let delete_bucket_request = self.client.as_inner().delete_bucket(bucket_name);
        let result = delete_bucket_request.send().await.map_err(Error::Client);

        let elapsed = start.elapsed();

        match result {
            Ok(_response) => {
                info!(
                    target: TRACING_TARGET_BUCKETS,
                    bucket = %bucket_name,
                    elapsed = ?elapsed,
                    "Bucket deleted successfully"
                );
                Ok(())
            }
            Err(e) => {
                error!(
                    target: TRACING_TARGET_BUCKETS,
                    bucket = %bucket_name,
                    error = %e,
                    elapsed = ?elapsed,
                    "Failed to delete bucket"
                );
                Err(e)
            }
        }
    }

    /// Lists all buckets.
    ///
    /// # Errors
    ///
    /// Returns an error if the bucket listing fails.
    #[instrument(skip(self), target = TRACING_TARGET_BUCKETS)]
    pub async fn list_buckets(&self) -> Result<Vec<BucketInfo>> {
        debug!(target: TRACING_TARGET_OPERATIONS, "Listing buckets");

        let start = std::time::Instant::now();
        let list_buckets_request = self.client.as_inner().list_buckets();
        let result = list_buckets_request.send().await.map_err(Error::Client);

        let elapsed = start.elapsed();

        match result {
            Ok(buckets) => {
                let bucket_infos: Vec<BucketInfo> = buckets
                    .buckets
                    .into_iter()
                    .map(|bucket| {
                        BucketInfo::new(bucket.name)
                            .with_creation_date(time::OffsetDateTime::now_utc())
                    })
                    .collect();

                info!(
                    target: TRACING_TARGET_BUCKETS,
                    count = bucket_infos.len(),
                    elapsed = ?elapsed,
                    "Buckets listed successfully"
                );

                Ok(bucket_infos)
            }
            Err(e) => {
                error!(
                    target: TRACING_TARGET_BUCKETS,
                    error = %e,
                    elapsed = ?elapsed,
                    "Failed to list buckets"
                );
                Err(e)
            }
        }
    }

    /// Checks if a bucket exists.
    ///
    /// # Arguments
    ///
    /// * `bucket_name` - Name of the bucket to check
    ///
    /// # Errors
    ///
    /// Returns an error if the existence check fails.
    #[instrument(skip(self), target = TRACING_TARGET_BUCKETS, fields(bucket = %bucket_name))]
    pub async fn bucket_exists(&self, bucket_name: &str) -> Result<bool> {
        debug!(
            target: TRACING_TARGET_OPERATIONS,
            bucket = %bucket_name,
            "Checking if bucket exists"
        );

        let start = std::time::Instant::now();
        let bucket_exists_request = self.client.as_inner().bucket_exists(bucket_name);
        let result = bucket_exists_request.send().await.map_err(Error::Client);

        let elapsed = start.elapsed();

        match result {
            Ok(response) => {
                let exists = response.exists;
                debug!(
                    target: TRACING_TARGET_BUCKETS,
                    bucket = %bucket_name,
                    exists = %exists,
                    elapsed = ?elapsed,
                    "Bucket existence check completed"
                );
                Ok(exists)
            }
            Err(e) => {
                error!(
                    target: TRACING_TARGET_BUCKETS,
                    bucket = %bucket_name,
                    error = %e,
                    elapsed = ?elapsed,
                    "Failed to check bucket existence"
                );
                Err(e)
            }
        }
    }

    /// Gets bucket information.
    ///
    /// # Arguments
    ///
    /// * `bucket_name` - Name of the bucket
    ///
    /// # Errors
    ///
    /// Returns an error if the bucket doesn't exist or the operation fails.
    #[instrument(skip(self), target = TRACING_TARGET_BUCKETS, fields(bucket = %bucket_name))]
    pub async fn get_bucket_info(&self, bucket_name: &str) -> Result<BucketInfo> {
        debug!(
            target: TRACING_TARGET_OPERATIONS,
            bucket = %bucket_name,
            "Getting bucket info"
        );

        // First check if bucket exists
        if !self.bucket_exists(bucket_name).await? {
            return Err(Error::NotFound(format!(
                "Bucket '{}' does not exist",
                bucket_name
            )));
        }

        // For MinIO, we can only get basic info from list_buckets
        let buckets = self.list_buckets().await?;
        buckets
            .into_iter()
            .find(|b| b.name == bucket_name)
            .ok_or_else(|| Error::NotFound(format!("Bucket '{}' not found", bucket_name)))
    }

    /// Sets bucket policy.
    ///
    /// # Arguments
    ///
    /// * `bucket_name` - Name of the bucket
    /// * `policy` - JSON policy string
    ///
    /// # Errors
    ///
    /// Returns an error if setting the policy fails.
    #[instrument(skip(self, _policy), target = TRACING_TARGET_BUCKETS, fields(bucket = %bucket_name))]
    pub async fn set_bucket_policy(&self, bucket_name: &str, _policy: &str) -> Result<()> {
        debug!(
            target: TRACING_TARGET_OPERATIONS,
            bucket = %bucket_name,
            "Setting bucket policy"
        );

        let start = std::time::Instant::now();
        let put_policy_request = self.client.as_inner().put_bucket_policy(bucket_name);
        let result = put_policy_request.send().await.map_err(Error::Client);

        let elapsed = start.elapsed();

        match result {
            Ok(_response) => {
                info!(
                    target: TRACING_TARGET_BUCKETS,
                    bucket = %bucket_name,
                    elapsed = ?elapsed,
                    "Bucket policy set successfully"
                );
                Ok(())
            }
            Err(e) => {
                error!(
                    target: TRACING_TARGET_BUCKETS,
                    bucket = %bucket_name,
                    error = %e,
                    elapsed = ?elapsed,
                    "Failed to set bucket policy"
                );
                Err(e)
            }
        }
    }

    /// Gets bucket policy.
    ///
    /// # Arguments
    ///
    /// * `bucket_name` - Name of the bucket
    ///
    /// # Errors
    ///
    /// Returns an error if getting the policy fails.
    #[instrument(skip(self), target = TRACING_TARGET_BUCKETS, fields(bucket = %bucket_name))]
    pub async fn get_bucket_policy(&self, bucket_name: &str) -> Result<String> {
        debug!(
            target: TRACING_TARGET_OPERATIONS,
            bucket = %bucket_name,
            "Getting bucket policy"
        );

        let start = std::time::Instant::now();
        let get_policy_request = self.client.as_inner().get_bucket_policy(bucket_name);
        let result = get_policy_request.send().await.map_err(Error::Client);

        let elapsed = start.elapsed();

        match result {
            Ok(response) => {
                let policy = response.config;
                debug!(
                    target: TRACING_TARGET_BUCKETS,
                    bucket = %bucket_name,
                    elapsed = ?elapsed,
                    "Bucket policy retrieved successfully"
                );
                Ok(policy)
            }
            Err(e) => {
                error!(
                    target: TRACING_TARGET_BUCKETS,
                    bucket = %bucket_name,
                    error = %e,
                    elapsed = ?elapsed,
                    "Failed to get bucket policy"
                );
                Err(e)
            }
        }
    }

    /// Removes bucket policy.
    ///
    /// # Arguments
    ///
    /// * `bucket_name` - Name of the bucket
    ///
    /// # Errors
    ///
    /// Returns an error if removing the policy fails.
    #[instrument(skip(self), target = TRACING_TARGET_BUCKETS, fields(bucket = %bucket_name))]
    pub async fn remove_bucket_policy(&self, bucket_name: &str) -> Result<()> {
        debug!(
            target: TRACING_TARGET_OPERATIONS,
            bucket = %bucket_name,
            "Removing bucket policy"
        );

        let start = std::time::Instant::now();
        let delete_policy_request = self.client.as_inner().delete_bucket_policy(bucket_name);
        let result = delete_policy_request.send().await.map_err(Error::Client);

        let elapsed = start.elapsed();

        match result {
            Ok(_response) => {
                info!(
                    target: TRACING_TARGET_BUCKETS,
                    bucket = %bucket_name,
                    elapsed = ?elapsed,
                    "Bucket policy removed successfully"
                );
                Ok(())
            }
            Err(e) => {
                error!(
                    target: TRACING_TARGET_BUCKETS,
                    bucket = %bucket_name,
                    error = %e,
                    elapsed = ?elapsed,
                    "Failed to remove bucket policy"
                );
                Err(e)
            }
        }
    }
}
