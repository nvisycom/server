# nvisy-runtime

Workflow definitions and execution engine for Nvisy pipelines.

This crate provides the core abstractions for defining and executing
data processing workflows as directed acyclic graphs (DAGs).

## Architecture

Workflows are represented as graphs with three types of nodes:

- **Source nodes**: Read or produce data (entry points)
- **Transformer nodes**: Process or transform data (intermediate)
- **Sink nodes**: Write or consume data (exit points)

## Example

```rust
use nvisy_runtime::prelude::*;

// Create a workflow graph
let mut graph = WorkflowGraph::new();

// Add nodes
let source = graph.add_node(SourceNode::new("s3_input", SourceKind::S3));
let transform = graph.add_node(TransformerNode::new("extract_text", TransformerKind::ExtractText));
let sink = graph.add_node(SinkNode::new("store_output", SinkKind::Database));

// Connect nodes
graph.connect(source, transform).unwrap();
graph.connect(transform, sink).unwrap();

// Validate the workflow
graph.validate().unwrap();
```

## Node Types

### Source Nodes
- `S3` - Amazon S3 compatible storage
- `Gcs` - Google Cloud Storage
- `AzureBlob` - Azure Blob Storage
- `GoogleDrive` - Google Drive
- `Dropbox` - Dropbox cloud storage
- `OneDrive` - Microsoft OneDrive
- `HttpUpload` - Receive files from HTTP upload
- `ApiEndpoint` - Fetch from an external API

### Transformer Nodes
- `ExtractText` - Extract text from documents
- `ChunkContent` - Split content into chunks
- `GenerateEmbeddings` - Generate vector embeddings
- `LlmTransform` - Transform using an LLM
- `ConvertFormat` - Convert file format
- `Validate` - Validate content against schema
- `Filter` - Filter data based on conditions
- `Merge` - Merge multiple inputs

### Sink Nodes
- `S3` - Amazon S3 compatible storage
- `Gcs` - Google Cloud Storage
- `AzureBlob` - Azure Blob Storage
- `GoogleDrive` - Google Drive
- `Dropbox` - Dropbox cloud storage
- `OneDrive` - Microsoft OneDrive
- `Database` - Store in database
- `VectorStore` - Store vector embeddings
- `Webhook` - Send to webhook
- `ApiEndpoint` - Send to external API
