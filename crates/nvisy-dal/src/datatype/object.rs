//! Object data type for files and binary objects.

use bytes::Bytes;
use serde::{Deserialize, Serialize};

use super::{DataType, Metadata};

/// An object representing a file or binary data (S3, GCS, Azure Blob).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Object {
    /// Path or key identifying this object.
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

impl DataType for Object {}

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
