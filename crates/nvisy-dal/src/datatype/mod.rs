//! Data types for the DAL.

mod document;
mod embedding;
mod graph;
mod message;
mod object;
mod record;

use std::collections::HashMap;

use derive_more::From;
use serde::{Deserialize, Serialize};

pub use document::Document;
pub use embedding::Embedding;
pub use graph::{Edge, Graph, Node};
pub use message::Message;
pub use object::Object;
pub use record::Record;

/// Metadata associated with data items.
pub type Metadata = HashMap<String, serde_json::Value>;

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
