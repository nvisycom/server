# Architecture

## System Design

The server is a single deployable: the API gateway and the processing engine
in one process. It handles authentication, authorization, workspace
management, document storage, credential encryption, and API serving, and it
embeds `elide-pipeline` from
[Elide Runtime](https://github.com/nvisycom/elide-runtime), the stateless
detect/redact library built on the [Elide](https://github.com/nvisycom/elide)
toolkit, for detection, redaction, and format processing.

The library owns no persistence and no HTTP layer, so the security boundary
(auth, encryption, tenant isolation) stays in the server. Model inference (NER,
OCR, speech-to-text, LLMs) runs in external inference services the engine
calls; the server configures them and stores their credentials. NATS carries
background work queues and status broadcasts between the server's own
components. See [Intelligence](./INTELLIGENCE.md) for how detection and
redaction run.

## Crate Structure

The Rust workspace contains these crates, each with a single responsibility:

| Crate                | Role                                                   |
| -------------------- | ------------------------------------------------------ |
| `nvisy-cli`          | Server entry point and CLI configuration               |
| `nvisy-core`         | Shared types, error handling, encryption utilities     |
| `nvisy-nats`         | NATS client for messaging, job queues, and KV state    |
| `nvisy-s3`           | First-party blob storage over an S3-compatible backend |
| `nvisy-object-store` | Providers for external object stores (S3, Azure, GCS)  |
| `nvisy-file-service` | OAuth-based cloud file services (Google Drive, ...)    |
| `nvisy-inference`    | LLM inference provider configuration and clients       |
| `nvisy-postgres`     | PostgreSQL ORM layer using Diesel with async support   |
| `nvisy-server`       | HTTP API handlers, middleware, authentication          |
| `nvisy-webhook`      | Webhook delivery for external event notification       |

## Technology Stack

| Layer      | Technology                                                         |
| ---------- | ------------------------------------------------------------------ |
| Language   | Rust                                                               |
| Database   | PostgreSQL for relational data                                     |
| Messaging  | NATS with JetStream for pub/sub and durable job queues             |
| Storage    | S3-compatible blob store (RustFS by default) for first-party blobs |
| HTTP       | Axum with Tower middleware, Aide for OpenAPI, Scalar for docs UI   |
| Auth       | JWT with Ed25519 signing, Argon2 for password hashing              |
| Encryption | XChaCha20-Poly1305 with HKDF-SHA256 key derivation                 |

## Deployment Models

The platform is deployment-agnostic. The surrounding infrastructure determines
which model applies:

- **Cloud-hosted:** Managed deployment in the vendor's infrastructure.
- **VPC deployment:** Installation within the customer's own virtual private
  cloud, ensuring data never leaves their network boundary.
- **On-premises:** Full deployment on customer-owned hardware for organizations
  with strict data sovereignty requirements.
- **Air-gapped:** Operation without network connectivity, required by certain
  government and defense use cases.
- **Edge processing:** Lightweight deployment at the point of data capture.

## Server Responsibilities

### Document Lifecycle

Documents are uploaded to the server via multipart HTTP or imported from
connected external systems. The server stores each document as an encrypted
binary object in the S3-compatible blob store, with metadata (type, size, hash,
version chain) tracked in PostgreSQL. Documents support versioning through
parent-child chains, allowing the original and redacted versions to coexist.

Detection analyzes a document in a background worker; redaction applies the
findings and stores the redacted file as a new document. All content processing
goes through the embedded engine; the rest of the server manages storage,
access control, and the encrypted-at-rest guarantee.

### Job Orchestration

Detection runs off the request thread. A request creates the detection and
enqueues its job in the same transaction (a transactional outbox); a drainer
relays pending jobs to a NATS JetStream work-queue, and the detection worker
consumes them, runs the engine, stores the encrypted audit, and records the
outcome. Each terminal transition is broadcast on a core-NATS status subject,
streamed to watchers over SSE, and emitted as a webhook event.

A worker claims a detection with a lease before analyzing it. A redelivered
job whose worker is still making progress is ignored; a detection whose worker
died is re-claimed and re-analyzed once the lease expires, so a detection is
never analyzed twice concurrently.

Redaction is synchronous: the request applies a completed detection's findings,
with any reviewer edits, and returns the redacted document.

### Credential Management

Connection and inference-provider credentials (API keys, connection strings,
storage access keys) are encrypted at rest using workspace-derived keys. The
server decrypts them only at the point of use: when a connection sync imports or
exports files, or when the engine calls a provider. Credentials are never
exposed through the API.

### Reviews

Reviewers can edit a detection before redacting it: suppress a false positive,
retag a detection, or add one the analysis missed. The stored analysis is never
modified; edits are layered onto a working copy at redaction time, so a
detection can be redacted many times. A review is a named discussion on a
document, with comments and a manual sign-off lifecycle, that references the
detections and redactions done for it. Live status (detection progress, unread
notification counts) is delivered over SSE.

## Data Model

The data model is organized around workspaces. A workspace is the tenant
boundary: all resources belong to exactly one workspace, and all access is
scoped accordingly.

**Workspaces** contain the following resource types:

- **Documents:** Binary objects stored in the S3-compatible blob store with
  metadata in PostgreSQL. Documents support versioning through parent-child chains and are
  classified by source: uploaded by a user, imported from an external system, or
  generated by a redaction operation. Documents can be deleted through the API;
  deletion is soft by default, with configurable retention policies that
  enforce permanent removal after expiry.

- **Connections:** Encrypted references to external systems. Each connection
  stores a provider identifier and an encrypted blob containing credentials and
  configuration. Encryption uses workspace-derived keys so that a compromise of
  one workspace cannot expose another's credentials.

- **Annotations:** Metadata attached to documents by users or by the engine's
  detection results. Annotations describe detected entities, their locations,
  triggering rules, and confidence levels. They serve both review workflows
  and audit reporting. Annotations support cursor-based pagination and
  individual access by ID.

- **Pipelines:** Processing workflow definitions with lifecycle status (draft,
  enabled, disabled) and optional cron scheduling. When a pipeline executes, it
  produces a **run**: an immutable record of the execution with a snapshot of
  the definition, execution logs, and timing. Runs produce **artifacts** that
  link back to documents.

- **Webhooks:** Event subscription endpoints configured per workspace. Each
  webhook specifies which event types to receive, a delivery URL, and a signing
  secret. The server delivers events with HMAC-SHA256 signatures, retries
  failed deliveries with exponential backoff, and exposes delivery attempt
  history through the API.

**Accounts** are the user-level identity:

- **Profile:** Account details (name, email) managed through the accounts API.
  Accounts can be deleted to satisfy GDPR right-to-erasure, which cascades
  removal of all associated data (memberships, files, annotations, tokens,
  notifications).

- **API Tokens:** Long-lived JWT tokens for programmatic access. Tokens can be
  created, listed, updated, and revoked. Each token can be scoped to specific
  workspaces and permission sets, enforcing the principle of least privilege.

- **Notifications:** Per-user notification feed for workspace events (member
  invitations, role changes, processing completions). Notifications are
  delivered in-app and optionally via email or webhook.

- **Activities:** Append-only audit feed of actions performed within
  workspaces. Activity records are immutable and tamper-evident, providing the
  queryable audit trail required for compliance reporting. Records include
  actor identity, action type, resource references, and timestamps.

**Workspace membership** is managed through:

- **Members:** Accounts assigned to a workspace with a role (Guest, Member,
  Admin, Owner). Members can be listed, removed, and have their roles changed.

- **Invites:** Email-based or code-based invitations to join a workspace.
  Invites carry an expiration and can be accepted, declined, or cancelled.

## Data Retention

The server enforces configurable retention policies for each resource type:

- **Original content:** Workspaces configure maximum retention periods after
  which original (pre-redaction) documents are permanently deleted. A
  zero-retention mode is available for environments where persistent storage
  of sensitive content is prohibited: originals are discarded immediately
  after processing completes.
- **Redacted output:** Retained independently of originals, subject to their
  own retention schedule.
- **Audit records:** Retained separately from content with longer retention
  periods to meet regulatory requirements (e.g., seven years for HIPAA, six
  years for SOX). Audit records are never deleted before their configured
  retention period expires, regardless of content deletion status.

Retention enforcement runs as a scheduled background task that identifies
expired resources and permanently removes them.

## Multi-Tenancy

### Tenant Isolation

The server enforces tenant isolation at the data layer: content, metadata, and
audit records are keyed by workspace identity, ensuring that no workspace can
access another's data. Credential encryption uses workspace-derived keys so
that a compromise of one workspace cannot expose another's secrets.

### Tenant-Specific Configuration

Each workspace maintains its own detection policies, redaction rules, entity
definitions, and connection credentials. Policies can extend prebuilt
regulation packs (HIPAA, GDPR, PCI-DSS, CCPA) with workspace-specific
additions.

## API Design

The HTTP API follows REST conventions with cursor-based pagination,
workspace-scoped resource creation, and role-based access control. All endpoints
are documented via OpenAPI and browsable through the Scalar UI at `/api/scalar`.

Resource paths follow two patterns: workspace-scoped creation and listing
(`/workspaces/{id}/resources/`) and direct access by ID (`/resources/{id}/`).
This avoids redundant workspace lookups when the resource ID is already known.

### Versioning

The API uses URI-based versioning (e.g., `/v1/workspaces/`). Breaking changes
require a major version increment with a documented deprecation timeline and
migration period. Non-breaking additions (new fields, new endpoints) are
introduced within the current version.

### Rate Limiting

The server enforces per-client and per-tenant rate limits at the middleware
layer. Limits are configurable per endpoint category (authentication, file
upload, job dispatch) and return standard `429 Too Many Requests` responses
with `Retry-After` headers.

### Idempotency

Mutating operations (file upload, job dispatch, webhook delivery) accept an
optional `Idempotency-Key` header. When provided, the server deduplicates
requests with the same key within a configurable window, returning the
original response. This ensures retry safety in distributed systems.

### API Surface

| Domain           | Endpoints                                                     |
| ---------------- | ------------------------------------------------------------- |
| Authentication   | Login, refresh, logout, OIDC sign-in (Google, Microsoft)      |
| Accounts         | Profile (read, update, delete), sign-in identities (password + linked providers), SCIM provisioning |
| API Tokens       | CRUD for scoped programmatic access tokens                    |
| Workspaces       | CRUD for tenant workspaces                                    |
| Members          | List, remove, role management, leave workspace                |
| Invites          | Email invites and shareable invite codes                       |
| Documents        | Upload, list, read, delete (with versioning and retention)    |
| Connections      | CRUD for encrypted provider credentials                       |
| Annotations      | CRUD with cursor pagination for document annotations          |
| Pipelines        | CRUD for processing workflows                                 |
| Pipeline Runs    | Read-only execution history and artifacts                     |
| Webhooks         | CRUD for event subscriptions, test delivery, delivery history |
| Notifications    | List, mark-as-read, delivery (in-app, email, webhook)        |
| Health           | Per-component system health status                            |

## Observability

### Metrics

The server exposes operational metrics covering ingestion throughput, detection
latency, queue depth, error rates, and resource utilization compatible with
standard monitoring systems (Prometheus, OpenTelemetry).

### Distributed Tracing

Each piece of content carries a trace identifier through every stage of the
pipeline: upload, queued detection, redaction, and result storage. All tracing events use explicit, hierarchical target names following
the convention `<crate>::<module>::<submodule>` (e.g.,
`nvisy_server::handler::contexts`). This enables precise per-module log
filtering in production without relying on log levels alone.

### Health Checks

The server exposes a `/health/` endpoint that returns per-component status
(PostgreSQL, NATS, blob store, and webhook delivery) with overall system health
(healthy, degraded, unhealthy).
Cached health checks serve load balancer probes; authenticated requests
trigger real-time checks.

## Security Model

**Authentication** uses JWT tokens signed with Ed25519. A sign-in method is
either a local password (hashed with Argon2) or a linked OIDC provider (Google,
Microsoft); both are stored as account identities, decoupled from the account,
and both issue the same JWT session.

**Authorization** is role-based, with permissions checked per workspace. Each
API operation requires a specific permission (e.g., manage connections, view
documents, upload files).

**Credential encryption** uses a two-tier key hierarchy. A master key derives
workspace-specific keys via HKDF-SHA256. Each workspace's connection credentials
are encrypted with XChaCha20-Poly1305 using its derived key. Encrypted data is
never exposed through the API.

See [Security](./SECURITY.md) for the full security model.
