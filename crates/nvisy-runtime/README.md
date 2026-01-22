# nvisy-runtime

Workflow definitions and execution engine for Nvisy pipelines.

This crate provides the core abstractions for defining and executing
data processing workflows as directed acyclic graphs (DAGs).

## Architecture

Workflows are represented as graphs with four types of nodes:

- **Input nodes**: Read or produce data (entry points)
- **Transform nodes**: Process or transform data (intermediate)
- **Output nodes**: Write or consume data (exit points)
- **Switch nodes**: Route data conditionally based on properties

### Definition vs Compiled Types

The crate separates workflow representation into two layers:

- **Definition types** (`graph::definition`): JSON-serializable types for
  storing, editing, and transmitting workflows. These include `WorkflowDefinition`,
  `NodeDef`, `InputDef`, `OutputDef`, and `CacheSlot`.

- **Compiled types** (`graph::compiled`): Runtime-optimized types for execution.
  These include `CompiledGraph`, `CompiledNode`, and processor types like
  `EmbeddingProcessor` and `EnrichProcessor`.

Use the `graph::compiler` module to transform definitions into executable graphs.

## Example

```rust,ignore
use nvisy_runtime::graph::definition::{
    InputDef, NodeDef, OutputDef, WorkflowDefinition,
};
use nvisy_runtime::graph::compiler::compile;
use nvisy_runtime::engine::Engine;
use nvisy_runtime::provider::CredentialsRegistry;

// Create a workflow definition
let mut workflow = WorkflowDefinition::new();

// Add input, transform, and output nodes...
// Connect nodes with edges...

// Compile the definition
let registry = CredentialsRegistry::default();
let ctx = nvisy_dal::core::Context::default();
let compiled = compile(workflow, &registry, ctx).await?;

// Execute the compiled graph
let engine = Engine::with_defaults();
let result = engine.execute_compiled(compiled, registry).await?;
```

## Node Types

### Input Nodes
Input nodes read data from external sources:
- Amazon S3, Google Cloud Storage, Azure Blob Storage
- PostgreSQL, MySQL databases

### Transform Nodes
- `Partition` - Extract elements from documents
- `Chunk` - Split content into smaller chunks
- `Embedding` - Generate vector embeddings
- `Enrich` - Add metadata/descriptions using LLMs
- `Extract` - Extract structured data or convert formats
- `Derive` - Generate new content (summaries, titles)

### Output Nodes
Output nodes write data to external destinations:
- Amazon S3, Google Cloud Storage, Azure Blob Storage
- PostgreSQL, MySQL databases
- Qdrant, Pinecone, Milvus, pgvector (vector databases)

### Switch Nodes
Route data based on conditions:
- Content type (image, document, text, etc.)
- File size thresholds
- Metadata presence/values
- File name patterns
