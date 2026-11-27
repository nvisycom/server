//! Blobs table constraint violations.

use strum::EnumString;

/// Blobs table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq, EnumString)]
pub enum WorkspaceBlobConstraints {
    #[strum(serialize = "blobs_content_hash_length")]
    ContentHashLength,
    #[strum(serialize = "blobs_file_size_min")]
    FileSizeMin,
    #[strum(serialize = "blobs_storage_path_not_empty")]
    StoragePathNotEmpty,
    #[strum(serialize = "blobs_storage_bucket_not_empty")]
    StorageBucketNotEmpty,
    #[strum(serialize = "blobs_ref_count_min")]
    RefCountMin,
    #[strum(serialize = "blobs_audit_digest_length")]
    AuditDigestLength,
    #[strum(serialize = "blobs_purged_after_created")]
    PurgedAfterCreated,
    #[strum(serialize = "blobs_reclaimed_after_purged")]
    ReclaimedAfterPurged,
}
