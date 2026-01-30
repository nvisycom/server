# Providers

## The Problem of Breadth

An ETL platform is only as useful as the systems it can connect to. Relational
databases, object stores, vector databases, document stores, message queues,
search engines, graph databases — each has its own wire protocol, authentication
model, pagination scheme, and SDK.

Nvisy addresses this with a provider abstraction that decouples the core
platform from specific external systems. The Rust server manages connections,
credentials, and pipeline orchestration. Provider integrations — the actual
reading from and writing to external systems — run in a separate TypeScript
runtime, communicating with the server over NATS.

## Provider Abstraction

Every external system is accessed through a uniform provider interface. A
provider is defined by three concerns:

**Connection** establishes a session with the external system using typed
credentials and parameters. Credentials are encrypted at rest and decrypted only
at execution time within the scope of a single run.

**Reading** streams data from the source with resumable pagination. Each item
carries its own context — a cursor, token, or offset — so that processing can
resume from any point, whether recovering from a failure or continuing
incrementally after new data has been added to the source.

**Writing** sends data to the sink in batches. The platform defines typed data
representations for each category of system: binary objects for file stores,
relational records for databases, vector embeddings for similarity search
systems, JSON documents for document stores, messages for queues, and graph
structures for graph databases.

## Connection Model

The Rust server stores connections as encrypted references to external systems.
Each connection record contains a provider identifier and an encrypted credential
blob. Encryption uses workspace-derived keys (HKDF-SHA256 + XChaCha20-Poly1305)
so that a compromise of one workspace cannot expose another's credentials.

At pipeline execution time, the server decrypts the connection credentials and
passes them to the TypeScript runtime over NATS. The runtime uses these
credentials to establish sessions with external systems and execute the pipeline
graph.

## Adding a New Provider

Adding a provider does not require modifying the Rust server. The process is:

1. Define the provider's connection parameters and credential schema
2. Implement the read and/or write interface in the TypeScript runtime
3. Register the provider identifier so the runtime can dispatch to it

This design keeps the provider surface area decoupled from the core. The server
does not know or care which specific systems are available — it manages
connections, credentials, and orchestration while the runtime handles execution.

## Design Principles

**Async-first.** All provider operations are asynchronous. No synchronous
wrappers, no blocking calls.

**Minimal coupling.** Providers share only the core data types and protocols.
Each provider is an independent module with its own dependencies.

**Validated boundaries.** Inputs are validated on both sides of the NATS
boundary. Errors include full context for debugging.
