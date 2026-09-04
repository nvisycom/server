//! The S3-compatible blob store.

use aws_sdk_s3::Client;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::types::{CompletedMultipartUpload, CompletedPart};
use tokio::io::{AsyncRead, AsyncReadExt};

use crate::error::{S3Error, S3Result};
use crate::key::ObjectKey;

/// Tracing target for blob-store operations.
const TRACING_TARGET: &str = "nvisy_s3::store";

/// Size of each multipart-upload part. S3 requires every part except the last to
/// be at least 5 MiB; 8 MiB keeps part counts low for large objects while staying
/// comfortably above the floor.
const PART_SIZE: usize = 8 * 1024 * 1024;

/// A handle to a stored object's content and size.
///
/// `into_reader` yields an [`AsyncRead`] over the object's bytes; `size` is the
/// object's length in bytes as reported by storage.
pub struct GetObject {
    reader: Box<dyn AsyncRead + Send + Unpin>,
    size: u64,
}

impl GetObject {
    /// The object size in bytes.
    #[must_use]
    pub fn size(&self) -> u64 {
        self.size
    }

    /// Consumes the handle, returning an [`AsyncRead`] over the object's bytes.
    #[must_use]
    pub fn into_reader(self) -> Box<dyn AsyncRead + Send + Unpin> {
        self.reader
    }
}

/// A first-party blob store backed by an S3-compatible service.
///
/// One S3 bucket holds every logical [`Bucket`](crate::Bucket); each object is
/// addressed by `"{bucket.prefix()}/{key}"`, and the bucket is derived from the
/// key's [`ObjectKey::BUCKET`]. Cloneable and cheap to pass around — it wraps an
/// `Arc`-backed SDK client.
#[derive(Clone)]
pub struct BlobStore {
    client: Client,
    bucket: String,
}

impl BlobStore {
    /// Wraps an already-built S3 client for `bucket`.
    pub(crate) fn new(client: Client, bucket: String) -> Self {
        Self { client, bucket }
    }

    /// The full S3 object key for `key`, namespaced under its store's prefix.
    fn object_key<K: ObjectKey>(key: &K) -> String {
        format!("{}/{}", K::BUCKET.prefix(), key)
    }

    /// Streams `reader` into `key`'s store, returning the number of bytes written.
    ///
    /// The target store is [`K::BUCKET`](ObjectKey::BUCKET), so a key can only be
    /// written to its own store. Uploads as a single S3 object when the content
    /// fits one part, and as a multipart upload otherwise, so an object of unknown
    /// length streams without being buffered whole in memory. A failure after the
    /// multipart upload begins aborts it so no partial upload lingers.
    pub async fn put<K, R>(&self, key: &K, reader: R) -> S3Result<u64>
    where
        K: ObjectKey,
        R: AsyncRead + Unpin + Send,
    {
        let object_key = Self::object_key(key);
        tracing::debug!(target: TRACING_TARGET, key = %object_key, "Uploading object");

        let mut reader = reader;
        let first = read_part(&mut reader).await?;

        // Small object: one PutObject, no multipart bookkeeping.
        if (first.len() as u64) < PART_SIZE as u64 {
            let size = first.len() as u64;
            self.client
                .put_object()
                .bucket(&self.bucket)
                .key(&object_key)
                .body(ByteStream::from(first))
                .send()
                .await
                .map_err(|err| S3Error::operation("put", err.into_service_error()))?;
            tracing::debug!(target: TRACING_TARGET, key = %object_key, size, "Object uploaded");
            return Ok(size);
        }

        self.put_multipart(&object_key, first, reader).await
    }

    /// Streams the remainder of `reader` after `first` as a multipart upload.
    async fn put_multipart<R>(
        &self,
        object_key: &str,
        first: Vec<u8>,
        mut reader: R,
    ) -> S3Result<u64>
    where
        R: AsyncRead + Unpin + Send,
    {
        let created = self
            .client
            .create_multipart_upload()
            .bucket(&self.bucket)
            .key(object_key)
            .send()
            .await
            .map_err(|err| S3Error::operation("put", err.into_service_error()))?;
        let upload_id = created.upload_id().ok_or_else(|| {
            S3Error::operation_msg("put", "S3 create_multipart_upload returned no upload id")
        })?;

        // Upload every part; on any failure, abort so no partial upload lingers.
        let result = self
            .upload_parts(object_key, upload_id, first, &mut reader)
            .await;
        let (parts, size) = match result {
            Ok(uploaded) => uploaded,
            Err(err) => {
                self.abort_multipart(object_key, upload_id).await;
                return Err(err);
            }
        };

        // Parts uploaded; completing them can still fail. Abort on that too, so a
        // failure never leaves the uploaded parts billing until S3 lifecycle reaps
        // them.
        let completed = self
            .client
            .complete_multipart_upload()
            .bucket(&self.bucket)
            .key(object_key)
            .upload_id(upload_id)
            .multipart_upload(
                CompletedMultipartUpload::builder()
                    .set_parts(Some(parts))
                    .build(),
            )
            .send()
            .await;
        if let Err(err) = completed {
            self.abort_multipart(object_key, upload_id).await;
            return Err(S3Error::operation("put", err.into_service_error()));
        }

        tracing::debug!(target: TRACING_TARGET, key = %object_key, size, "Object uploaded (multipart)");
        Ok(size)
    }

    /// Uploads `first` and every subsequent part read from `reader`, returning the
    /// completed parts and the total byte count.
    async fn upload_parts<R>(
        &self,
        object_key: &str,
        upload_id: &str,
        first: Vec<u8>,
        reader: &mut R,
    ) -> S3Result<(Vec<CompletedPart>, u64)>
    where
        R: AsyncRead + Unpin + Send,
    {
        let mut parts = Vec::new();
        let mut total: u64 = 0;
        let mut part = first;
        let mut part_number = 1;

        loop {
            total += part.len() as u64;
            let uploaded = self
                .client
                .upload_part()
                .bucket(&self.bucket)
                .key(object_key)
                .upload_id(upload_id)
                .part_number(part_number)
                .body(ByteStream::from(part))
                .send()
                .await
                .map_err(|err| S3Error::operation("put", err.into_service_error()))?;
            parts.push(
                CompletedPart::builder()
                    .part_number(part_number)
                    .set_e_tag(uploaded.e_tag().map(str::to_owned))
                    .build(),
            );

            // The next part; an empty read means the stream is exhausted.
            let next = read_part(reader).await?;
            if next.is_empty() {
                break;
            }
            part = next;
            part_number += 1;
        }

        Ok((parts, total))
    }

    /// Aborts a multipart upload, best-effort — a failure only leaves an
    /// incomplete upload for S3's own lifecycle rules to reap, so it is logged
    /// rather than propagated over the original error.
    async fn abort_multipart(&self, object_key: &str, upload_id: &str) {
        if let Err(err) = self
            .client
            .abort_multipart_upload()
            .bucket(&self.bucket)
            .key(object_key)
            .upload_id(upload_id)
            .send()
            .await
        {
            tracing::warn!(
                target: TRACING_TARGET,
                key = %object_key,
                error = %err.into_service_error(),
                "Failed to abort a multipart upload; left for S3 lifecycle cleanup",
            );
        }
    }

    /// Fetches an object as a stream, or `None` if it does not exist.
    pub async fn get<K: ObjectKey>(&self, key: &K) -> S3Result<Option<GetObject>> {
        let object_key = Self::object_key(key);
        tracing::debug!(target: TRACING_TARGET, key = %object_key, "Fetching object");

        let output = match self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(&object_key)
            .send()
            .await
        {
            Ok(output) => output,
            Err(err) => {
                let service = err.into_service_error();
                if service.is_no_such_key() {
                    return Ok(None);
                }
                return Err(S3Error::operation("get", service));
            }
        };

        let size = output.content_length().unwrap_or_default().max(0) as u64;
        let reader = output.body.into_async_read();
        Ok(Some(GetObject {
            reader: Box::new(reader),
            size,
        }))
    }

    /// Deletes an object. Idempotent: deleting a missing object succeeds.
    pub async fn delete<K: ObjectKey>(&self, key: &K) -> S3Result<()> {
        let object_key = Self::object_key(key);
        tracing::debug!(target: TRACING_TARGET, key = %object_key, "Deleting object");

        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(&object_key)
            .send()
            .await
            .map_err(|err| S3Error::operation("delete", err.into_service_error()))?;
        Ok(())
    }

    /// Probes the store's liveness by heading the configured bucket.
    ///
    /// Succeeds when the endpoint is reachable and the bucket exists and is
    /// accessible with the current credentials — the same preconditions every
    /// object operation needs, so it doubles as a readiness check.
    pub async fn ping(&self) -> S3Result<()> {
        self.client
            .head_bucket()
            .bucket(&self.bucket)
            .send()
            .await
            .map_err(|err| S3Error::operation("head_bucket", err.into_service_error()))?;
        Ok(())
    }

    /// Whether an object exists, via a HEAD request.
    pub async fn exists<K: ObjectKey>(&self, key: &K) -> S3Result<bool> {
        let object_key = Self::object_key(key);
        match self
            .client
            .head_object()
            .bucket(&self.bucket)
            .key(&object_key)
            .send()
            .await
        {
            Ok(_) => Ok(true),
            Err(err) => {
                let service = err.into_service_error();
                if service.is_not_found() {
                    Ok(false)
                } else {
                    Err(S3Error::operation("head", service))
                }
            }
        }
    }
}

/// Reads up to [`PART_SIZE`] bytes from `reader`, returning fewer only at EOF.
///
/// An empty return means the stream is exhausted. Reading the whole part up front
/// is what lets a small object take the single-`PutObject` path and lets each
/// multipart part meet S3's minimum size.
async fn read_part<R: AsyncRead + Unpin>(reader: &mut R) -> S3Result<Vec<u8>> {
    let mut buffer = Vec::with_capacity(PART_SIZE);
    let mut chunk = [0u8; 64 * 1024];
    while buffer.len() < PART_SIZE {
        let read = reader
            .read(&mut chunk)
            .await
            .map_err(|err| S3Error::Body(err.to_string()))?;
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..read]);
    }
    Ok(buffer)
}
