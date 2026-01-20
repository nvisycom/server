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

impl Message {
    /// Creates a new message.
    pub fn new(id: impl Into<String>, payload: impl Into<Bytes>) -> Self {
        Self {
            id: id.into(),
            payload: payload.into(),
            headers: HashMap::new(),
            timestamp: None,
        }
    }

    /// Sets a header.
    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }

    /// Sets the timestamp.
    pub fn with_timestamp(mut self, timestamp: Timestamp) -> Self {
        self.timestamp = Some(timestamp);
        self
    }

    /// Tries to deserialize the payload as JSON.
    pub fn payload_json<T: serde::de::DeserializeOwned>(&self) -> Result<T, serde_json::Error> {
        serde_json::from_slice(&self.payload)
    }
}

impl DataType for Message {
    const TYPE_ID: &'static str = "message";

    fn data_type_id() -> super::DataTypeId {
        super::DataTypeId::Message
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
