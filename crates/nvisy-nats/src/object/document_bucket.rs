//! Document bucket configuration for NATS object storage.

use std::time::Duration;

use strum::{AsRefStr, Display, EnumString, IntoStaticStr};

/// Document storage bucket types.
///
/// Defines the available buckets for storing document files in NATS object storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Display, EnumString, AsRefStr, IntoStaticStr)]
#[strum(serialize_all = "snake_case")]
pub enum DocumentBucket {
    /// Primary document storage for uploaded and processed files.
    /// No expiration, files are retained indefinitely.
    Files,
    /// Temporary storage for intermediate processing artifacts.
    /// Files expire after 7 days.
    Intermediates,
}

impl DocumentBucket {
    /// Returns the bucket name.
    pub fn name(&self) -> &'static str {
        self.into()
    }

    /// Returns the maximum age for objects in this bucket, if any.
    ///
    /// Returns `None` for buckets where objects should not expire.
    pub fn max_age(&self) -> Option<Duration> {
        match self {
            Self::Files => None,
            Self::Intermediates => Some(Duration::from_secs(7 * 24 * 60 * 60)), // 7 days
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bucket_names() {
        assert_eq!(DocumentBucket::Files.name(), "files");
        assert_eq!(DocumentBucket::Intermediates.name(), "intermediates");
    }
}
