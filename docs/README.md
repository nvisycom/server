# Nvisy

## Overview

Nvisy is an open-source ETL platform for building intelligent data pipelines. It
connects to external data sources and sinks through a pluggable provider system,
transforms data through AI-powered and rule-based processors, and orchestrates
execution as compiled workflow graphs.

The platform is designed around three ideas: data should flow between any two
systems without custom glue code, transformations should be composable and
reusable, and the intelligence applied during processing — extraction,
enrichment, analysis, reasoning — should be a first-class part of the pipeline,
not a bolted-on afterthought.

## Problem

Organizations accumulate data across dozens of systems — relational databases,
object stores, vector databases, document repositories, message queues. Moving
data between these systems typically requires either rigid, vendor-locked ETL
tools that cannot accommodate AI workloads, or bespoke engineering that is
expensive to build and maintain.

Nvisy addresses this by providing a declarative workflow language that compiles
to an optimized execution graph. Users define what data to read, how to
transform it, and where to write the results. The platform handles connection
management, credential security, incremental streaming, and execution
orchestration.

## Design Principles

**Workflow-first.** The pipeline is the unit of work. Each workflow is a
directed acyclic graph of typed nodes — sources, transforms, and sinks —
connected by edges. This structure is serializable, versionable, and
schedulable.

**Provider-agnostic.** A uniform interface abstracts over relational databases,
object stores, vector databases, and other external systems. Adding a new
provider requires implementing a small set of protocols without modifying the
core engine.

**Intelligence-native.** LLM-powered transforms — extraction, enrichment,
summarization, entity resolution, contradiction detection — sit alongside
rule-based transforms as equal citizens in the graph. AI is not a separate
layer; it is woven into the data flow.

**Resumable streaming.** Every data item carries its own pagination context.
Runs can resume from the last processed item — whether recovering from a failure
or continuing incrementally after new data has been added to the source.

**Workspace isolation.** Tenants are cryptographically isolated. Provider
credentials are encrypted with workspace-derived keys, and all data access is
scoped to the workspace boundary.

## Documentation

| Document                          | Description                                                  |
| --------------------------------- | ------------------------------------------------------------ |
| [Architecture](./ARCHITECTURE.md) | System design, pipeline model, and technology stack          |
| [Intelligence](./INTELLIGENCE.md) | AI-powered transform and analysis capabilities               |
| [Providers](./PROVIDERS.md)       | Data provider architecture and the Rust-Python bridge        |
| [Security](./SECURITY.md)         | Authentication, encryption, authorization, and audit logging |

## Deployment

Infrastructure requirements, configuration reference, and Docker setup are
documented in the [`docker/`](../docker/) directory.
