//! Pipeline-run blob I/O.
//!
//! [`ArtifactWriter`] stages a run's outputs and [`ArtifactReader`] loads them
//! back, both in the platform's first-party S3-compatible blob store (the
//! `Documents`, `Audits`, and `Intermediates` [`Bucket`]s), handling
//! per-workspace encryption and the blob bookkeeping each one needs. They are
//! distinct from [`ExternalObjectStore`], which bridges external tenant object
//! stores. Staging and loading are disjoint concerns over the same two clients,
//! so they are two services rather than one: most callers touch only one side.
//!
//! Bytes are content-addressed: each `stage_*` method writes the (encrypted)
//! object and returns a [`NewBlob`] describing it, which the caller resolves
//! through [`find_or_create_blob`] (directly or via
//! [`create_workspace_document`]) so identical content is stored once and shared.
//!
//! The crypto-free GC counterpart lives with its only caller, the reaper
//! (`worker::reaper`): reclaiming an object needs no workspace key.
//!
//! [`Bucket`]: nvisy_s3::Bucket
//! [`ExternalObjectStore`]: crate::service::ExternalObjectStore
//! [`NewBlob`]: nvisy_postgres::model::NewBlob
//! [`find_or_create_blob`]: nvisy_postgres::query::WorkspaceBlobRepository::find_or_create_blob
//! [`create_workspace_document`]: nvisy_postgres::query::WorkspaceDocumentRepository::create_workspace_document

mod reader;
mod writer;

use nvisy_s3::{AuditKey, BlobStore, Bucket, DocumentKey, IntermediateKey, ObjectKey};

use crate::response::{ErrorKind, Result};
pub use crate::service::blob::reader::ArtifactReader;
pub use crate::service::blob::writer::{ArtifactWriter, StagedBlob};

/// Removes a blob's object from whichever store `bucket` names.
///
/// An unknown bucket or an unparseable storage path is an error, not a silent
/// success: the object was not removed, so its blob stays pending. Avatars are
/// not blob-tracked, so a blob naming that store is an error too.
pub(crate) async fn delete_object(
    blobs: &BlobStore,
    bucket: &str,
    storage_path: &str,
) -> Result<()> {
    async fn delete<K: ObjectKey>(blobs: &BlobStore, storage_path: &str) -> Result<()> {
        let key = K::from_str(storage_path).map_err(|err| {
            ErrorKind::InternalServerError
                .with_message("Invalid blob storage key")
                .with_context(err.to_string())
        })?;
        Ok(blobs.delete(&key).await?)
    }

    match Bucket::from_name(bucket) {
        Some(Bucket::Documents) => delete::<DocumentKey>(blobs, storage_path).await,
        Some(Bucket::Audits) => delete::<AuditKey>(blobs, storage_path).await,
        Some(Bucket::Intermediates) => delete::<IntermediateKey>(blobs, storage_path).await,
        Some(Bucket::Avatars) | None => Err(ErrorKind::InternalServerError
            .with_message("Blob references a store that holds no blobs")
            .with_context(format!("bucket: {bucket}"))),
    }
}
