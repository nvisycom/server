//! Blob data type for files and objects.

use bytes::Bytes;
use serde::{Deserialize, Serialize};

use super::{DataType, Metadata};

/// A blob representing a file or object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Blob {
    /// Path or key identifying this blob.
    pub path: String,
    /// Raw binary data.
    #[serde(with = "serde_bytes")]
    pub data: Bytes,
    /// Content type (MIME type).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    /// Additional metadata.
    #[serde(default)]
    pub metadata: Metadata,
}

impl Blob {
    /// Creates a new blob.
    pub fn new(path: impl Into<String>, data: impl Into<Bytes>) -> Self {
        Self {
            path: path.into(),
            data: data.into(),
            content_type: None,
            metadata: Metadata::new(),
        }
    }

    /// Sets the content type.
    pub fn with_content_type(mut self, content_type: impl Into<String>) -> Self {
        self.content_type = Some(content_type.into());
        self
    }

    /// Sets metadata.
    pub fn with_metadata(mut self, metadata: Metadata) -> Self {
        self.metadata = metadata;
        self
    }
}

impl DataType for Blob {
    const TYPE_ID: &'static str = "blob";

    fn data_type_id() -> super::DataTypeId {
        super::DataTypeId::Blob
    }
}

mod serde_bytes {
    use bytes::Bytes;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(bytes: &Bytes, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        bytes.as_ref().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Bytes, D::Error>
    where
        D: Deserializer<'de>,
    {
        let vec = Vec::<u8>::deserialize(deserializer)?;
        Ok(Bytes::from(vec))
    }
}
