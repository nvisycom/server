//! Object bucket configuration for NATS object storage.

use std::time::Duration;

/// Marker trait for object storage buckets.
///
/// This trait defines the configuration for a NATS object storage bucket,
/// including its name and optional TTL for objects.
pub trait ObjectBucket: Clone + Send + Sync + 'static {
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
    const MAX_AGE: Option<Duration> = None;
    const NAME: &'static str = "DOCUMENT_FILES";
}

/// Temporary storage for intermediate processing artifacts.
///
/// Files expire after 7 days.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct IntermediatesBucket;

impl ObjectBucket for IntermediatesBucket {
    const MAX_AGE: Option<Duration> = Some(Duration::from_secs(7 * 24 * 60 * 60));
    const NAME: &'static str = "DOCUMENT_INTERMEDIATES";
}

/// Storage for document thumbnails.
///
/// No expiration, thumbnails are retained indefinitely.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ThumbnailsBucket;

impl ObjectBucket for ThumbnailsBucket {
    const MAX_AGE: Option<Duration> = None;
    const NAME: &'static str = "DOCUMENT_THUMBNAILS";
}

/// Storage for account avatars.
///
/// No expiration, avatars are retained indefinitely.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct AvatarsBucket;

impl ObjectBucket for AvatarsBucket {
    const MAX_AGE: Option<Duration> = None;
    const NAME: &'static str = "ACCOUNT_AVATARS";
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
        assert_eq!(
            IntermediatesBucket::MAX_AGE,
            Some(Duration::from_secs(7 * 24 * 60 * 60))
        );
        assert_eq!(ThumbnailsBucket::MAX_AGE, None);
        assert_eq!(AvatarsBucket::MAX_AGE, None);
    }
}
