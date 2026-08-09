//! Object bucket configuration for NATS object storage.

use std::time::Duration;

use super::object_key::{
    AccountAvatarKey, FileKey, IntermediateKey, ObjectKey, WorkspaceAvatarKey,
};

/// Marker trait for object storage buckets.
///
/// This trait defines the configuration for a NATS object storage bucket,
/// including its name, optional TTL, and the key type that addresses its
/// objects.
pub trait ObjectBucket: Clone + Send + Sync + 'static {
    /// Key type that addresses objects in this bucket.
    type Key: ObjectKey;

    /// Bucket name used in NATS object storage.
    const NAME: &'static str;

    /// Maximum age for objects in this bucket.
    /// Returns `None` for buckets where objects should not expire.
    const MAX_AGE: Option<Duration>;
}

/// Primary file storage for uploaded and processed files.
///
/// No expiration, files are retained indefinitely.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct FilesBucket;

impl ObjectBucket for FilesBucket {
    type Key = FileKey;

    const MAX_AGE: Option<Duration> = None;
    const NAME: &'static str = "DOCUMENT_FILES";
}

/// Storage for intermediate processing artifacts.
///
/// Holds a run's analyzed document between the detect and redact calls, plus
/// any other between-phase artifacts. Retained for the life of the run that
/// owns it: no expiration, since the run row references the object indefinitely
/// and redaction may happen long after detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct IntermediatesBucket;

impl ObjectBucket for IntermediatesBucket {
    type Key = IntermediateKey;

    const MAX_AGE: Option<Duration> = None;
    const NAME: &'static str = "DOCUMENT_INTERMEDIATES";
}

/// Storage for document thumbnails.
///
/// No expiration, thumbnails are retained indefinitely.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ThumbnailsBucket;

impl ObjectBucket for ThumbnailsBucket {
    type Key = FileKey;

    const MAX_AGE: Option<Duration> = None;
    const NAME: &'static str = "DOCUMENT_THUMBNAILS";
}

/// Storage for account avatars.
///
/// No expiration, avatars are retained indefinitely.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct AvatarsBucket;

impl ObjectBucket for AvatarsBucket {
    type Key = AccountAvatarKey;

    const MAX_AGE: Option<Duration> = None;
    const NAME: &'static str = "ACCOUNT_AVATARS";
}

/// Storage for workspace avatars (logos).
///
/// No expiration, avatars are retained indefinitely.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct WorkspaceAvatarsBucket;

impl ObjectBucket for WorkspaceAvatarsBucket {
    type Key = WorkspaceAvatarKey;

    const MAX_AGE: Option<Duration> = None;
    const NAME: &'static str = "WORKSPACE_AVATARS";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bucket_names() {
        assert_eq!(FilesBucket::NAME, "DOCUMENT_FILES");
        assert_eq!(IntermediatesBucket::NAME, "DOCUMENT_INTERMEDIATES");
        assert_eq!(ThumbnailsBucket::NAME, "DOCUMENT_THUMBNAILS");
        assert_eq!(AvatarsBucket::NAME, "ACCOUNT_AVATARS");
    }

    #[test]
    fn test_bucket_max_age() {
        assert_eq!(FilesBucket::MAX_AGE, None);
        assert_eq!(IntermediatesBucket::MAX_AGE, None);
        assert_eq!(ThumbnailsBucket::MAX_AGE, None);
        assert_eq!(AvatarsBucket::MAX_AGE, None);
    }
}
