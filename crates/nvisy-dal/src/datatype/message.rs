//! Message data type for queue messages.

use std::collections::HashMap;

use bytes::Bytes;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};

use super::DataType;

/// A message from a queue or stream.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Unique identifier.
    pub id: String,
    /// Message payload.
    #[serde(with = "serde_bytes")]
    pub payload: Bytes,
    /// Message headers.
    #[serde(default)]
    pub headers: HashMap<String, String>,
    /// Timestamp when the message was created.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<Timestamp>,
}

impl DataType for Message {}

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
