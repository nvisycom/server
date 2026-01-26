//! Data types for the DAL.
//!
//! These types represent the data items that flow through providers:
//! - `Object` for object storage (S3, GCS, Azure Blob)
//! - `Document` for JSON documents
//! - `Embedding` for vector embeddings
//! - `Record` for relational rows
//! - `Message` for queue/stream messages
//! - `Graph`, `Node`, `Edge` for graph data

use std::collections::HashMap;

use bytes::Bytes;
use derive_more::From;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Metadata associated with data items.
pub type Metadata = HashMap<String, Value>;

/// Marker trait for data types that can be read/written through the DAL.
pub trait DataType: Send + Sync + 'static {}

/// Type-erased data value for runtime dispatch.
#[derive(Debug, Clone, From, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum AnyDataValue {
    /// Object storage item (S3, GCS, etc.).
    Object(Object),
    /// JSON document.
    Document(Document),
    /// Vector embedding.
    Embedding(Embedding),
    /// Graph with nodes and edges.
    Graph(Graph),
    /// Relational record/row.
    Record(Record),
    /// Queue/stream message.
    Message(Message),
}

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

/// A document with flexible JSON content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    /// Unique identifier.
    pub id: String,
    /// Document content as JSON.
    pub content: Value,
    /// Additional metadata.
    #[serde(default)]
    pub metadata: Metadata,
}

impl DataType for Document {}

/// A vector embedding with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Embedding {
    /// Unique identifier.
    pub id: String,
    /// The embedding vector.
    pub vector: Vec<f32>,
    /// Additional metadata.
    #[serde(default)]
    pub metadata: Metadata,
}

impl DataType for Embedding {}

/// A record representing a row in a relational table.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Record {
    /// Column values keyed by column name.
    pub columns: HashMap<String, Value>,
}

impl DataType for Record {}

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

/// A graph containing nodes and edges.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Graph {
    /// Nodes in the graph.
    #[serde(default)]
    pub nodes: Vec<Node>,
    /// Edges in the graph.
    #[serde(default)]
    pub edges: Vec<Edge>,
}

impl DataType for Graph {}

/// A node in a graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    /// Unique identifier.
    pub id: String,
    /// Node labels (types).
    #[serde(default)]
    pub labels: Vec<String>,
    /// Node properties.
    #[serde(default)]
    pub properties: HashMap<String, Value>,
}

/// An edge in a graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    /// Unique identifier.
    pub id: String,
    /// Source node ID.
    pub from: String,
    /// Target node ID.
    pub to: String,
    /// Edge label (relationship type).
    pub label: String,
    /// Edge properties.
    #[serde(default)]
    pub properties: HashMap<String, Value>,
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
