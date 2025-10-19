//! Bucket information structures for MinIO storage.
//!
//! This module provides data structures for representing bucket metadata
//! and properties in MinIO object storage.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// Information about a MinIO bucket.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BucketInfo {
    /// Bucket name.
    pub name: String,
    /// Bucket creation date.
    pub creation_date: Option<OffsetDateTime>,
    /// Bucket region.
    pub region: Option<String>,
    /// Bucket tags.
    pub tags: HashMap<String, String>,
}

impl BucketInfo {
    /// Creates a new BucketInfo.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            creation_date: None,
            region: None,
            tags: HashMap::new(),
        }
    }

    /// Sets the creation date.
    pub fn with_creation_date(mut self, creation_date: OffsetDateTime) -> Self {
        self.creation_date = Some(creation_date);
        self
    }

    /// Sets the region.
    pub fn with_region(mut self, region: impl Into<String>) -> Self {
        self.region = Some(region.into());
        self
    }

    /// Sets tags.
    pub fn with_tags(mut self, tags: HashMap<String, String>) -> Self {
        self.tags = tags;
        self
    }
}
