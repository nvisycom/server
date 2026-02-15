# Architecture

## System Design

Nvisy is implemented as a workspace-based monorepo in Rust. The server handles
HTTP serving, database access, messaging, and pipeline orchestration. Pipeline
execution — including AI transforms, provider integrations, and data
processing — runs in a separate TypeScript runtime, communicating with the
server over NATS.

## Crate Structure

The Rust workspace contains six crates, each with a single responsibility:

| Crate            | Role                                                       |
| ---------------- | ---------------------------------------------------------- |
| `nvisy-cli`      | Server entry point and CLI configuration                   |
| `nvisy-core`     | Shared types, error handling, encryption utilities         |
| `nvisy-nats`     | NATS client for messaging, job queues, and object storage  |
| `nvisy-postgres` | PostgreSQL ORM layer using Diesel with async support       |
| `nvisy-server`   | HTTP API handlers, middleware, authentication              |
| `nvisy-webhook`  | Webhook delivery for external event notification           |

## Technology Stack

| Layer      | Technology                                                         |
| ---------- | ------------------------------------------------------------------ |
| Language   | Rust                                                               |
| Database   | PostgreSQL with pgvector for relational data and vector embeddings |
| Messaging  | NATS with JetStream for pub/sub, job queues, and object storage    |
| HTTP       | Axum with Tower middleware, Aide for OpenAPI, Scalar for docs UI   |
| Auth       | JWT with Ed25519 signing, Argon2 for password hashing              |
| Encryption | XChaCha20-Poly1305 with HKDF-SHA256 key derivation                 |

## Pipeline Model

A Nvisy pipeline is a directed acyclic graph. Nodes represent operations —
reading data, transforming it, writing results — and edges define the flow
between them. This graph is the central abstraction of the platform: users
author it as a JSON document, the compiler validates and optimizes it, and the
engine executes it.

The graph model makes pipelines inspectable, versionable, and composable.
Because the structure is data, not code, it can be stored in a database,
rendered in a visual editor, diffed between versions, and snapshotted at
execution time for auditability.

### Node Types

Every node in the graph falls into one of four categories:

**Source nodes** read data from external systems — relational databases, object
stores, or other providers connected through the platform's credential
management system. Each source references a stored connection and carries
provider-specific parameters.

**Transform nodes** process data in flight. Transforms range from rule-based
operations (partitioning, chunking) to AI-powered operations (extraction,
enrichment, embedding, derivation, entity resolution, contradiction detection).
See [Intelligence](./INTELLIGENCE.md) for the full catalog. Each transform
implements a uniform interface: accept input, produce output.

**Sink nodes** write data to external systems using the same connection and
provider abstraction as sources. A single pipeline can write to multiple sinks —
for example, storing extracted records in a relational database while
simultaneously writing embeddings to a vector store.

**Switch nodes** route data conditionally. A switch evaluates a condition
against each incoming item and directs it to one of two output branches. This
enables type-specific processing within a single pipeline — for example, routing
images through OCR while routing text through NLP extraction.

### Cache Slots

Some workflows require data to pass between branches that are not directly
connected. Cache slots provide named connection points for this purpose. A
transform writes to a named slot; another node reads from it. During
compilation, the system resolves these slots into direct graph edges,
eliminating the indirection at runtime.

### Compilation

Before execution, the workflow definition is compiled into an optimized runtime
graph. Compilation proceeds in four phases:

1. **Validation** verifies structural correctness — all edge references resolve
   to existing nodes, at least one source and one sink exist, and the graph is
   acyclic.

2. **Cache resolution** connects named cache slots. Outputs writing to a slot
   are wired directly to inputs reading from the same slot, and the intermediate
   cache nodes are removed from the graph.

3. **Node compilation** converts each definition node into its executable form —
   source nodes become input streams, sink nodes become output streams,
   transforms become processors, and switches become condition evaluators.

4. **Graph construction** builds the final directed graph with topological
   ordering support, ready for the execution engine.

### Execution

The engine processes compiled graphs using a streaming, item-at-a-time model.
For each item read from a source, the engine pushes it through every downstream
transform in topological order, then writes the result to all connected sinks
before advancing to the next item.

This design avoids buffering entire datasets in memory. A single document can
expand into thousands of chunks at one transform and contract back into a single
summary at another — the engine handles both directions naturally.

Every item carries its own resumption context. Runs can resume from the last
successfully processed item — whether recovering from a failure or continuing
incrementally after new data has been added to the source. This makes pipelines
suitable for both batch reprocessing and ongoing incremental ingestion.

### Scheduling and Triggers

Pipelines can be triggered in three ways: manually by a user, automatically by a
source event (such as a new file arriving), or on a cron schedule with timezone
support. Each trigger creates an independent run with its own execution log and
artifact set.

## Data Model

The data model is organized around workspaces. A workspace is the tenant
boundary — all resources belong to exactly one workspace, and all access is
scoped accordingly.

**Workspaces** contain three primary resource types:

- **Pipelines** — Workflow definitions stored as JSON graphs. Each pipeline has
  a lifecycle status (draft, enabled, disabled) and optional cron scheduling.
  When a pipeline executes, it produces a **run** — an immutable record of the
  execution with a snapshot of the definition, execution logs, and timing. Runs
  produce **artifacts**, which link back to files and classify them as input,
  output, or intermediate.

- **Connections** — Encrypted references to external systems. Each connection
  stores a provider identifier and an encrypted blob containing credentials and
  configuration. Encryption uses workspace-derived keys so that a compromise of
  one workspace cannot expose another's credentials.

- **Files** — Binary objects stored in NATS object storage with metadata in
  PostgreSQL. Files support versioning through parent-child chains and are
  classified by source: uploaded by a user, imported from an external system, or
  generated by a pipeline run.

## API Design

The HTTP API follows REST conventions with cursor-based pagination,
workspace-scoped resource creation, and role-based access control. All endpoints
are documented via OpenAPI and browsable through the Scalar UI at `/api/scalar`.

Resource paths follow two patterns: workspace-scoped creation and listing
(`/workspaces/{id}/resources/`) and direct access by ID (`/resources/{id}/`).
This avoids redundant workspace lookups when the resource ID is already known.

## Security Model

**Authentication** uses JWT tokens signed with Ed25519. Passwords are hashed
with Argon2.

**Authorization** is role-based, with permissions checked per workspace. Each
API operation requires a specific permission (e.g., create pipelines, view
connections, upload files).

**Credential encryption** uses a two-tier key hierarchy. A master key derives
workspace-specific keys via HKDF-SHA256. Each workspace's connection credentials
are encrypted with XChaCha20-Poly1305 using its derived key. Encrypted data is
never exposed through the API.
