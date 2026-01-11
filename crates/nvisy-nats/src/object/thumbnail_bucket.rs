//! Thumbnail bucket constants for NATS object storage.

use std::time::Duration;

/// Bucket name for document thumbnails.
pub const THUMBNAIL_BUCKET: &str = "DOCUMENT_THUMBNAILS";

/// Maximum age for thumbnails (none - retained indefinitely).
pub const THUMBNAIL_MAX_AGE: Option<Duration> = None;
