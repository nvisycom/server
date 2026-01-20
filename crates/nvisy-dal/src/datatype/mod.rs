//! Data types for the DAL.

mod blob;
mod document;
mod embedding;
mod graph;
mod message;
mod record;

use std::collections::HashMap;

pub use blob::Blob;
use derive_more::From;
pub use document::Document;
pub use embedding::Embedding;
pub use graph::{Edge, Graph, Node};
pub use message::Message;
pub use record::Record;
use serde::{Deserialize, Serialize};

/// Metadata associated with data items.
pub type Metadata = HashMap<String, serde_json::Value>;

/// Marker trait for data types that can be read/written through the DAL.
pub trait DataType: Send + Sync + 'static {
    /// Unique type identifier.
    const TYPE_ID: &'static str;

    /// Returns the corresponding DataTypeId.
    fn data_type_id() -> DataTypeId;
}

/// Data type identifier for runtime type checking and JSON schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataTypeId {
    Blob,
    Document,
    Embedding,
    Graph,
    Record,
    Message,
}

impl DataTypeId {
    /// Returns the string identifier for this type.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Blob => "blob",
            Self::Document => "document",
            Self::Embedding => "embedding",
            Self::Graph => "graph",
            Self::Record => "record",
            Self::Message => "message",
        }
    }
}

impl std::fmt::Display for DataTypeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Type-erased data value for runtime dispatch.
#[derive(Debug, Clone, From, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum AnyDataValue {
    Blob(Blob),
    Document(Document),
    Embedding(Embedding),
    Graph(Graph),
    Record(Record),
    Message(Message),
}

impl AnyDataValue {
    /// Returns the type identifier for this value.
    pub const fn type_id(&self) -> DataTypeId {
        match self {
            Self::Blob(_) => DataTypeId::Blob,
            Self::Document(_) => DataTypeId::Document,
            Self::Embedding(_) => DataTypeId::Embedding,
            Self::Graph(_) => DataTypeId::Graph,
            Self::Record(_) => DataTypeId::Record,
            Self::Message(_) => DataTypeId::Message,
        }
    }

    /// Attempts to extract a Blob value.
    pub fn into_blob(self) -> Option<Blob> {
        match self {
            Self::Blob(v) => Some(v),
            _ => None,
        }
    }

    /// Attempts to extract a Document value.
    pub fn into_document(self) -> Option<Document> {
        match self {
            Self::Document(v) => Some(v),
            _ => None,
        }
    }

    /// Attempts to extract an Embedding value.
    pub fn into_embedding(self) -> Option<Embedding> {
        match self {
            Self::Embedding(v) => Some(v),
            _ => None,
        }
    }

    /// Attempts to extract a Graph value.
    pub fn into_graph(self) -> Option<Graph> {
        match self {
            Self::Graph(v) => Some(v),
            _ => None,
        }
    }

    /// Attempts to extract a Record value.
    pub fn into_record(self) -> Option<Record> {
        match self {
            Self::Record(v) => Some(v),
            _ => None,
        }
    }

    /// Attempts to extract a Message value.
    pub fn into_message(self) -> Option<Message> {
        match self {
            Self::Message(v) => Some(v),
            _ => None,
        }
    }
}
