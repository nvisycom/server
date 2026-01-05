//! Document bucket configuration for NATS object storage.

use std::time::Duration;

/// Marker trait for document storage buckets.
///
/// This trait defines the configuration for a NATS object storage bucket,
/// including its name and optional TTL for objects.
pub trait DocumentBucket: Clone + Send + Sync + 'static {
    /// Bucket name used in NATS object storage.
    const NAME: &'static str;

    /// Maximum age for objects in this bucket.
    /// Returns `None` for buckets where objects should not expire.
    const MAX_AGE: Option<Duration>;
}

/// Primary document storage for uploaded and processed files.
///
/// No expiration, files are retained indefinitely.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Files;

impl DocumentBucket for Files {
    const MAX_AGE: Option<Duration> = None;
    const NAME: &'static str = "DOCUMENT_FILES";
}

/// Temporary storage for intermediate processing artifacts.
///
/// Files expire after 7 days.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Intermediates;

impl DocumentBucket for Intermediates {
    const MAX_AGE: Option<Duration> = Some(Duration::from_secs(7 * 24 * 60 * 60));
    const NAME: &'static str = "DOCUMENT_INTERMEDIATES"; // 7 days
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bucket_names() {
        assert_eq!(Files::NAME, "DOCUMENT_FILES");
        assert_eq!(Intermediates::NAME, "DOCUMENT_INTERMEDIATES");
    }

    #[test]
    fn test_bucket_max_age() {
        assert_eq!(Files::MAX_AGE, None);
        assert_eq!(
            Intermediates::MAX_AGE,
            Some(Duration::from_secs(7 * 24 * 60 * 60))
        );
    }
}
