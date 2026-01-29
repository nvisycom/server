# Providers

## The Problem of Breadth

An ETL platform is only as useful as the systems it can connect to. Relational
databases, object stores, vector databases, document stores, message queues,
search engines, graph databases — each has its own wire protocol, authentication
model, pagination scheme, and SDK.

Not all of these systems offer Rust as a first-class target for their client
libraries. The Python ecosystem, by contrast, has mature, well-maintained SDKs
for virtually every data system in production use today. The AI infrastructure
ecosystem is even more skewed — model providers, embedding services, and
orchestration frameworks overwhelmingly target Python first.

Nvisy addresses this with a dual-language architecture: the performance-critical
core — HTTP serving, workflow compilation, execution orchestration, encryption —
is written in Rust, while provider integrations are implemented in Python and
loaded at runtime through a PyO3 bridge. This gives the platform Rust's
performance and safety guarantees where they matter most, and Python's ecosystem
reach where breadth matters most.

## Provider Abstraction

Every external system is accessed through a uniform provider interface. A
provider is defined by three concerns:

**Connection** establishes a session with the external system using typed
credentials and parameters. Credentials are encrypted at rest and decrypted only
at execution time within the scope of a single run.

**Reading** streams data from the source with resumable pagination. Each item
carries its own context — a cursor, token, or offset — so that processing can
resume from any point, whether recovering from a failure or continuing
incrementally after new data has been added to the source. The platform defines
context types for different pagination strategies: marker-based for object
stores, keyset-based for relational databases, and token-based for vector
databases.

**Writing** sends data to the sink in batches. The platform defines typed data
representations for each category of system: binary objects for file stores,
relational records for databases, vector embeddings for similarity search
systems, JSON documents for document stores, messages for queues, and graph
structures for graph databases.

## Type Safety Across the Boundary

Rust defines the canonical types — data representations, parameter schemas,
context structures — and Python conforms to them through Pydantic models that
mirror the Rust structs. This ensures that data crossing the language boundary
is validated on both sides.

The Rust DAL crate defines three core traits:

- **Provider** — connect to and disconnect from an external system
- **DataInput** — read a resumable stream of typed items
- **DataOutput** — write a batch of typed items

Python providers implement matching structural protocols. The PyO3 bridge
handles conversion between Rust and Python representations, async interop
between Tokio and Python coroutines, and error propagation with full tracebacks.

## Adding a New Provider

Adding a provider does not require modifying the core engine. The process is:

1. Define credential and parameter types in the Rust DAL crate
2. Register the type-erased variants so the runtime can dispatch to the new
   provider
3. Add matching Pydantic models to the Python package
4. Implement the read and/or write protocols in a Python module
5. The runtime discovers and loads the provider by name at execution time

This design keeps the provider surface area decoupled from the core. The engine
does not know or care which specific systems are available — it operates on
abstract streams of typed data.

## Design Principles

**Async-first.** All provider operations are asynchronous. No synchronous
wrappers, no blocking calls. The GIL is released during all I/O operations to
maintain concurrency.

**Protocols over inheritance.** Python providers implement structural protocols
rather than inheriting from base classes. This allows any conforming object to
serve as a provider without coupling to a specific class hierarchy.

**Minimal coupling.** Providers share only the core data types and protocols.
Each provider is an independent module with its own optional dependencies,
installable separately.

**Validated boundaries.** Inputs are validated by Pydantic on the Python side
and by typed deserialization on the Rust side. Errors include full context —
Python tracebacks propagate through the bridge for debugging.
