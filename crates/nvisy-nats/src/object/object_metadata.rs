//! Object metadata for NATS object storage.

use std::collections::HashSet;

use jiff::Timestamp;
use serde::{Deserialize, Serialize};

use super::content_source::ContentSource;

/// Metadata associated with an object in storage.
///
/// This struct contains all the metadata information about a stored object,
/// including content hash, timestamps, version information, and custom attributes.
/// All fields are optional to allow for flexible metadata management.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[derive(Serialize, Deserialize)]
pub struct ObjectMetadata {
    /// SHA256 hash of the object content
    pub sha256: Option<String>,
    /// When the object was created
    pub created_at: Option<Timestamp>,
    /// When the object was last updated
    pub updated_at: Option<Timestamp>,
    /// Version of the object
    pub version: Option<u64>,
    /// Content source identifier
    pub content_source: ContentSource,
    /// Custom tags/labels for the object
    pub tags: Option<HashSet<String>>,
    /// Original filename if available
    pub original_filename: Option<String>,
}

impl ObjectMetadata {
    /// Creates a new instance of [`ObjectMetadata`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the SHA256 hash of the content.
    pub fn with_sha256(mut self, sha256: impl Into<String>) -> Self {
        self.sha256 = Some(sha256.into());
        self
    }

    /// Sets the created timestamp.
    pub fn with_created_at(mut self, created_at: Timestamp) -> Self {
        self.created_at = Some(created_at);
        self
    }

    /// Sets the updated timestamp.
    pub fn with_updated_at(mut self, updated_at: Timestamp) -> Self {
        self.updated_at = Some(updated_at);
        self
    }

    /// Sets the version of the object.
    pub fn with_version(mut self, version: u64) -> Self {
        self.version = Some(version);
        self
    }

    /// Sets the content source.
    pub fn with_content_source(mut self, content_source: ContentSource) -> Self {
        self.content_source = content_source;
        self
    }

    /// Sets custom tags for the object.
    pub fn with_tags(mut self, tags: HashSet<String>) -> Self {
        self.tags = Some(tags);
        self
    }

    /// Adds a single tag to the object.
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        match &mut self.tags {
            Some(tags) => {
                tags.insert(tag.into());
            }
            None => {
                let mut tags = HashSet::new();
                tags.insert(tag.into());
                self.tags = Some(tags);
            }
        }
        self
    }

    /// Sets the original filename.
    pub fn with_original_filename(mut self, filename: impl Into<String>) -> Self {
        self.original_filename = Some(filename.into());
        self
    }

    /// Sets the created timestamp to now.
    pub fn with_created_now(mut self) -> Self {
        self.created_at = Some(Timestamp::now());
        self
    }

    /// Sets the updated timestamp to now.
    pub fn with_updated_now(mut self) -> Self {
        self.updated_at = Some(Timestamp::now());
        self
    }

    /// Sets both created and updated timestamps to now.
    pub fn with_timestamps_now(mut self) -> Self {
        let now = Timestamp::now();
        self.created_at = Some(now);
        self.updated_at = Some(now);
        self
    }

    /// Gets the SHA256 hash.
    pub fn sha256(&self) -> Option<&str> {
        self.sha256.as_deref()
    }

    /// Gets the created timestamp.
    pub fn created_at(&self) -> Option<Timestamp> {
        self.created_at
    }

    /// Gets the updated timestamp.
    pub fn updated_at(&self) -> Option<Timestamp> {
        self.updated_at
    }

    /// Gets the version.
    pub fn version(&self) -> Option<u64> {
        self.version
    }

    /// Gets the content source.
    pub fn content_source(&self) -> ContentSource {
        self.content_source
    }

    /// Gets the tags.
    pub fn tags(&self) -> Option<&HashSet<String>> {
        self.tags.as_ref()
    }

    /// Gets a mutable reference to the tags.
    pub fn tags_mut(&mut self) -> &mut Option<HashSet<String>> {
        &mut self.tags
    }

    /// Gets the original filename.
    pub fn original_filename(&self) -> Option<&str> {
        self.original_filename.as_deref()
    }

    /// Updates the updated_at timestamp to now and increments version.
    pub fn touch(&mut self) {
        self.updated_at = Some(Timestamp::now());
        self.version = Some(self.version.unwrap_or(0) + 1);
    }

    /// Increments the version number.
    pub fn increment_version(&mut self) {
        self.version = Some(self.version.unwrap_or(0) + 1);
    }

    /// Checks if the metadata has any fields set (excluding content_source which is always present).
    pub fn is_empty(&self) -> bool {
        self.sha256.is_none()
            && self.created_at.is_none()
            && self.updated_at.is_none()
            && self.version.is_none()
            && self.tags.as_ref().is_none_or(|t| t.is_empty())
            && self.original_filename.is_none()
    }

    /// Adds a tag to the existing tags set.
    pub fn add_tag(&mut self, tag: impl Into<String>) {
        match &mut self.tags {
            Some(tags) => {
                tags.insert(tag.into());
            }
            None => {
                let mut tags = HashSet::new();
                tags.insert(tag.into());
                self.tags = Some(tags);
            }
        }
    }

    /// Removes a tag from the tags set.
    pub fn remove_tag(&mut self, tag: &str) -> bool {
        self.tags.as_mut().is_some_and(|tags| tags.remove(tag))
    }

    /// Checks if a tag exists.
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.as_ref().is_some_and(|tags| tags.contains(tag))
    }

    /// Clears all tags.
    pub fn clear_tags(&mut self) {
        if let Some(tags) = &mut self.tags {
            tags.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_object_metadata_default() {
        let metadata = ObjectMetadata::default();

        assert!(metadata.is_empty());
        assert_eq!(metadata.sha256(), None);
        assert_eq!(metadata.version(), None);
        // content_source should have a default UUID when using Default::default()
        assert_eq!(metadata.tags(), None);
    }

    #[test]
    fn test_object_metadata_builder_pattern() {
        let content_source = ContentSource::new();
        let metadata = ObjectMetadata::new()
            .with_sha256("hash123")
            .with_version(5)
            .with_content_source(content_source)
            .with_tag("env:production")
            .with_tag("team:backend")
            .with_original_filename("test.txt")
            .with_timestamps_now();

        assert_eq!(metadata.sha256(), Some("hash123"));
        assert_eq!(metadata.version(), Some(5));
        assert_eq!(metadata.content_source(), content_source);
        assert_eq!(metadata.original_filename(), Some("test.txt"));
        assert!(metadata.created_at().is_some());
        assert!(metadata.updated_at().is_some());

        let tags = metadata.tags().unwrap();
        assert!(tags.contains("env:production"));
        assert!(tags.contains("team:backend"));
        assert_eq!(tags.len(), 2);
    }

    #[test]
    fn test_object_metadata_touch() {
        let mut metadata = ObjectMetadata::new().with_version(1);
        let original_version = metadata.version();

        metadata.touch();

        assert!(metadata.updated_at().is_some());
        assert_eq!(metadata.version(), Some(original_version.unwrap() + 1));
    }

    #[test]
    fn test_tag_operations() {
        let mut metadata = ObjectMetadata::new();

        // Add tags
        metadata.add_tag("test1");
        metadata.add_tag("test2");

        assert!(metadata.has_tag("test1"));
        assert!(metadata.has_tag("test2"));
        assert!(!metadata.has_tag("test3"));

        // Remove tag
        assert!(metadata.remove_tag("test1"));
        assert!(!metadata.has_tag("test1"));
        assert!(!metadata.remove_tag("nonexistent"));

        // Clear tags
        metadata.clear_tags();
        assert!(!metadata.has_tag("test2"));
    }

    #[test]
    fn test_with_tags_hashset() {
        let mut tags = HashSet::new();
        tags.insert("tag1".to_string());
        tags.insert("tag2".to_string());

        let metadata = ObjectMetadata::new().with_tags(tags.clone());

        assert_eq!(metadata.tags(), Some(&tags));
    }

    #[test]
    fn test_increment_version() {
        let mut metadata = ObjectMetadata::new();

        // First increment on empty version
        metadata.increment_version();
        assert_eq!(metadata.version(), Some(1));

        // Second increment
        metadata.increment_version();
        assert_eq!(metadata.version(), Some(2));
    }

    #[test]
    fn test_is_empty() {
        let mut metadata = ObjectMetadata::new();
        assert!(metadata.is_empty());

        metadata.add_tag("test");
        assert!(!metadata.is_empty());

        metadata.clear_tags();
        assert!(metadata.is_empty());

        metadata = metadata.with_sha256("hash");
        assert!(!metadata.is_empty());
    }
}
