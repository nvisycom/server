//! Node data types representing different processing operations.

use serde::{Deserialize, Serialize};

/// Data associated with a workflow node.
///
/// Nodes are categorized by their role in data flow:
/// - **Source**: Reads/produces data (entry points)
/// - **Transformer**: Processes/transforms data (intermediate)
/// - **Sink**: Writes/consumes data (exit points)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NodeData {
    /// Data source node - reads or produces data.
    Source(SourceNode),
    /// Data transformer node - processes or transforms data.
    Transformer(TransformerNode),
    /// Data sink node - writes or consumes data.
    Sink(SinkNode),
}

impl NodeData {
    /// Returns the node's display name.
    pub fn name(&self) -> &str {
        match self {
            NodeData::Source(n) => &n.name,
            NodeData::Transformer(n) => &n.name,
            NodeData::Sink(n) => &n.name,
        }
    }

    /// Returns whether this is a source node.
    pub const fn is_source(&self) -> bool {
        matches!(self, NodeData::Source(_))
    }

    /// Returns whether this is a transformer node.
    pub const fn is_transformer(&self) -> bool {
        matches!(self, NodeData::Transformer(_))
    }

    /// Returns whether this is a sink node.
    pub const fn is_sink(&self) -> bool {
        matches!(self, NodeData::Sink(_))
    }
}

/// A data source node that reads or produces data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceNode {
    /// Display name of the source.
    pub name: String,
    /// Type of source.
    pub kind: SourceKind,
    /// Source-specific configuration.
    #[serde(default)]
    pub config: serde_json::Value,
}

/// Types of data sources.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    /// Amazon S3 compatible storage.
    S3,
    /// Google Cloud Storage.
    Gcs,
    /// Azure Blob Storage.
    AzureBlob,
    /// Google Drive.
    GoogleDrive,
    /// Dropbox.
    Dropbox,
    /// OneDrive.
    OneDrive,
    /// Receive files from HTTP upload.
    HttpUpload,
    /// Fetch from an external API.
    ApiEndpoint,
    /// Custom source type.
    Custom(String),
}

/// A data transformer node that processes or transforms data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransformerNode {
    /// Display name of the transformer.
    pub name: String,
    /// Type of transformation.
    pub kind: TransformerKind,
    /// Transformer-specific configuration.
    #[serde(default)]
    pub config: serde_json::Value,
}

/// Types of data transformations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransformerKind {
    /// Extract text from documents (PDF, images via OCR).
    ExtractText,
    /// Split content into chunks.
    ChunkContent,
    /// Generate vector embeddings.
    GenerateEmbeddings,
    /// Transform using an LLM.
    LlmTransform,
    /// Convert file format.
    ConvertFormat,
    /// Validate content against schema.
    Validate,
    /// Filter data based on conditions.
    Filter,
    /// Merge multiple inputs.
    Merge,
    /// Custom transformation.
    Custom(String),
}

/// A data sink node that writes or consumes data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SinkNode {
    /// Display name of the sink.
    pub name: String,
    /// Type of sink.
    pub kind: SinkKind,
    /// Sink-specific configuration.
    #[serde(default)]
    pub config: serde_json::Value,
}

/// Types of data sinks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SinkKind {
    /// Amazon S3 compatible storage.
    S3,
    /// Google Cloud Storage.
    Gcs,
    /// Azure Blob Storage.
    AzureBlob,
    /// Google Drive.
    GoogleDrive,
    /// Dropbox.
    Dropbox,
    /// OneDrive.
    OneDrive,
    /// Store in database.
    Database,
    /// Store vector embeddings.
    VectorStore,
    /// Send to webhook.
    Webhook,
    /// Send to external API.
    ApiEndpoint,
    /// Custom sink type.
    Custom(String),
}

impl SourceNode {
    /// Creates a new source node.
    pub fn new(name: impl Into<String>, kind: SourceKind) -> Self {
        Self {
            name: name.into(),
            kind,
            config: serde_json::Value::Object(Default::default()),
        }
    }

    /// Sets the configuration.
    pub fn with_config(mut self, config: serde_json::Value) -> Self {
        self.config = config;
        self
    }
}

impl TransformerNode {
    /// Creates a new transformer node.
    pub fn new(name: impl Into<String>, kind: TransformerKind) -> Self {
        Self {
            name: name.into(),
            kind,
            config: serde_json::Value::Object(Default::default()),
        }
    }

    /// Sets the configuration.
    pub fn with_config(mut self, config: serde_json::Value) -> Self {
        self.config = config;
        self
    }
}

impl SinkNode {
    /// Creates a new sink node.
    pub fn new(name: impl Into<String>, kind: SinkKind) -> Self {
        Self {
            name: name.into(),
            kind,
            config: serde_json::Value::Object(Default::default()),
        }
    }

    /// Sets the configuration.
    pub fn with_config(mut self, config: serde_json::Value) -> Self {
        self.config = config;
        self
    }
}

// Conversions to NodeData

impl From<SourceNode> for NodeData {
    fn from(node: SourceNode) -> Self {
        NodeData::Source(node)
    }
}

impl From<TransformerNode> for NodeData {
    fn from(node: TransformerNode) -> Self {
        NodeData::Transformer(node)
    }
}

impl From<SinkNode> for NodeData {
    fn from(node: SinkNode) -> Self {
        NodeData::Sink(node)
    }
}
