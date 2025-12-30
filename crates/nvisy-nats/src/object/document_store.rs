//! Document file stores for input, intermediate, and output files.

use async_nats::jetstream;
use derive_more::{Deref, DerefMut};
use uuid::Uuid;

use super::content_data::ContentData;
use super::object_headers::ObjectHeaders;
use super::object_key::{DocumentLabel, ObjectKey};
use super::object_key_data::ObjectKeyData;
use super::object_metadata::ObjectMetadata;
use super::object_store::{ObjectStore, PutResult};
use crate::Result;

/// A document file store that manages files for a specific document label.
///
/// This store uses `ObjectKey<S>` for type-safe file operations where S is the document label.
/// It uses Deref/DerefMut to delegate to the underlying ObjectStore. It automatically
/// creates rich metadata and headers for stored content based on the data characteristics.
///
/// # Type Parameters
///
/// * `S` - The document label type (Input, Intermediate, or Output)
#[derive(Clone, Deref, DerefMut)]
pub struct DocumentFileStore<S: DocumentLabel> {
    #[deref]
    #[deref_mut]
    store: ObjectStore<ObjectKey<S>>,
}

impl<S: DocumentLabel> DocumentFileStore<S> {
    /// Create a new document file store for a specific stage
    pub async fn new(jetstream: &jetstream::Context) -> Result<Self> {
        let store = ObjectStore::new(
            jetstream,
            S::bucket_name(),
            Some(S::description()),
            Some(S::max_age()),
        )
        .await?;

        Ok(Self { store })
    }

    /// Create a key for a new file in this store
    pub fn create_key(&self, workspace_uuid: Uuid, file_uuid: Uuid) -> ObjectKey<S> {
        let data = ObjectKeyData::new(workspace_uuid, file_uuid);
        data.build::<S>()
            .expect("Valid ObjectKeyData should build successfully")
    }

    /// Put ContentData into the store
    pub async fn put(
        &self,
        key: &ObjectKey<S>,
        data: &ContentData,
    ) -> Result<PutResult<ObjectKey<S>>> {
        self.store.put(key, data).await
    }

    /// Get ContentData from the store
    pub async fn get(&self, key: &ObjectKey<S>) -> Result<Option<ContentData>> {
        self.store.get(key).await
    }

    /// Create ContentData with basic metadata populated
    pub fn create_content_data_with_metadata(data: bytes::Bytes) -> ContentData {
        let content_data = ContentData::new(data);

        let sha256_hex = content_data.sha256_hex();
        let size = content_data.size();
        let is_text = content_data.is_likely_text();

        // Create basic metadata
        let metadata = ObjectMetadata::new()
            .with_sha256(sha256_hex.clone())
            .with_timestamps_now()
            .with_version(1);

        // Create basic headers
        let content_type = if is_text {
            "text/plain; charset=utf-8"
        } else {
            "application/octet-stream"
        };

        let headers = ObjectHeaders::new()
            .set("content-type", content_type)
            .set("content-length", size.to_string())
            .set("etag", sha256_hex);

        content_data.with_metadata(metadata).with_headers(headers)
    }

    /// List all files in this store for a specific workspace
    pub async fn list_workspace_files(&self, workspace_uuid: Uuid) -> Result<Vec<String>> {
        // Get all files in the store
        let all_files = self.store.list().await?;

        // Filter by workspace
        let prefix = ObjectKeyData::create_prefix(workspace_uuid);

        Ok(all_files
            .into_iter()
            .filter(|name| name.starts_with(&prefix))
            .collect())
    }

    /// Get the underlying object store
    pub fn inner(&self) -> &ObjectStore<ObjectKey<S>> {
        &self.store
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::InputFiles;

    #[test]
    fn test_create_key() {
        let workspace_uuid = Uuid::new_v4();
        let file_uuid = Uuid::new_v4();

        let data = ObjectKeyData::new(workspace_uuid, file_uuid);

        let key = data.build::<InputFiles>().unwrap();

        assert_eq!(key.workspace_uuid().unwrap(), workspace_uuid);
        assert_eq!(key.file_uuid().unwrap(), file_uuid);
    }

    #[test]
    fn test_create_content_data_with_metadata() {
        let text_data = DocumentFileStore::<InputFiles>::create_content_data_with_metadata(
            bytes::Bytes::from("Hello, world!"),
        );
        let binary_data = DocumentFileStore::<InputFiles>::create_content_data_with_metadata(
            bytes::Bytes::from(vec![0xFF, 0xFE, 0xFD, 0xFC]),
        );

        // Test text content metadata
        assert_eq!(
            text_data.metadata().sha256(),
            Some(text_data.sha256_hex()).as_deref()
        );
        assert_eq!(
            text_data.headers().get("content-type"),
            Some("text/plain; charset=utf-8")
        );
        assert_eq!(text_data.metadata().version(), Some(1));

        // Test binary content metadata
        assert_eq!(
            binary_data.metadata().sha256(),
            Some(binary_data.sha256_hex()).as_deref()
        );
        assert_eq!(
            binary_data.headers().get("content-type"),
            Some("application/octet-stream")
        );
        assert_eq!(binary_data.metadata().version(), Some(1));
    }

    #[test]
    fn test_metadata_and_headers_consistency() {
        let data = DocumentFileStore::<InputFiles>::create_content_data_with_metadata(
            bytes::Bytes::from("Test data for consistency check"),
        );

        // Verify consistency between metadata and headers
        assert_eq!(data.metadata().sha256(), data.headers().get("etag"));
        assert_eq!(
            data.headers().get("content-length"),
            Some(&data.size().to_string()).map(|s| s.as_str())
        );
        assert!(data.metadata().created_at().is_some());
        assert!(data.metadata().updated_at().is_some());
    }
}
