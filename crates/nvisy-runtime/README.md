# nvisy-runtime

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/server/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/server/actions/workflows/build.yml)

Workflow definitions and execution engine for Nvisy pipelines.

This crate provides the core abstractions for defining and executing
data processing workflows as directed acyclic graphs (DAGs).

## Architecture

### Definition vs Compiled Types

The crate separates workflow representation into two layers:

- **Definition types** (`definition`): JSON-serializable types for
  storing, editing, and transmitting workflows. These include `Workflow`,
  `Node`, `NodeKind`, `Input`, `Output`, and `CacheSlot`.

- **Compiled types** (`graph`): Runtime-optimized types for execution.
  These include `CompiledGraph`, `CompiledNode`, and processor types like
  `EmbeddingProcessor` and `EnrichProcessor`.

Use the `Engine` to compile definitions and execute workflows.

## Example

```rust,ignore
use nvisy_runtime::definition::{
    Input, Node, NodeKind, Output, Workflow,
};
use nvisy_runtime::engine::Engine;
use nvisy_runtime::provider::CredentialsRegistry;

// Create a workflow definition
let mut workflow = Workflow::new();

// Add input, transform, and output nodes...
// Connect nodes with edges...

// Execute the workflow
let engine = Engine::with_defaults();
let registry = CredentialsRegistry::default();
let ctx = nvisy_dal::core::Context::default();
let result = engine.execute(workflow, registry, ctx).await?;
```
