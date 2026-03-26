# Architecture

## System Design

The Nvisy platform consists of two components: the **server** (this
repository) and the **runtime**. The server is the API gateway: it handles
authentication, authorization, workspace management, document storage,
credential encryption, and API serving. The runtime handles detection,
redaction, format processing, and policy evaluation. The two communicate over
NATS.

This separation keeps the security boundary (auth, encryption, tenant
isolation) in the server, and the processing logic (ML inference, OCR,
redaction) in the runtime. The server never processes document content
directly; it stores, routes, and protects it.

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
binary object in NATS object storage, with metadata (type, size, hash, version
chain) tracked in PostgreSQL. Documents support versioning through
parent-child chains, allowing the original and redacted versions to coexist.

When a redaction job is requested, the server decrypts the document, passes it
to the runtime over NATS, and stores the redacted result as a new version. The
server never interprets document content; it manages storage, access control,
and the encrypted-at-rest guarantee.

### Job Orchestration

The server dispatches processing jobs to the runtime over NATS JetStream. Each
job carries the document reference, the workspace's detection policies, and
any review decisions. The runtime processes the job and returns results through
NATS. The server records the outcome, stores artifacts, and emits webhook
events.

Each job carries a request-scoped deadline. If the runtime does not return
results within the deadline, the job is marked as timed out and the caller is
notified. This prevents long-running redaction jobs from blocking resources
indefinitely.

### Credential Management

Provider credentials (API keys, connection strings, storage access keys) are
encrypted at rest using workspace-derived keys. At processing time, the server
decrypts credentials and passes them to the runtime over NATS. The runtime
uses these credentials to establish sessions with external systems. Credentials
are never exposed through the API.

### Studio Sessions

Studio provides an interactive editing environment for reviewing and refining
redaction results. Users can accept, reject, or modify detected entities
before finalizing the redacted document. Studio sessions use WebSocket
connections backed by NATS pub/sub for real-time collaboration. The server
manages session state and access control; the runtime handles the actual
redaction operations.

## Data Model

The data model is organized around workspaces. A workspace is the tenant
boundary: all resources belong to exactly one workspace, and all access is
scoped accordingly.

**Workspaces** contain the following resource types:

- **Documents:** Binary objects stored in NATS object storage with metadata in
  PostgreSQL. Documents support versioning through parent-child chains and are
  classified by source: uploaded by a user, imported from an external system, or
  generated by a redaction operation. Documents can be deleted through the API;
  deletion is soft by default, with configurable retention policies that
  enforce permanent removal after expiry.

- **Connections:** Encrypted references to external systems. Each connection
  stores a provider identifier and an encrypted blob containing credentials and
  configuration. Encryption uses workspace-derived keys so that a compromise of
  one workspace cannot expose another's credentials.

- **Context Files:** Workspace-scoped configuration files stored as encrypted
  JSON in NATS object storage. These provide additional context for detection
  policies: custom entity definitions, domain-specific terminology, and
  redaction rules.

- **Annotations:** Metadata attached to documents by users or by the runtime's
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
| Authentication   | Login, refresh, logout, SSO (SAML/OIDC)                       |
| Accounts         | Profile (read, update, delete), SCIM provisioning             |
| API Tokens       | CRUD for scoped programmatic access tokens                    |
| Workspaces       | CRUD for tenant workspaces                                    |
| Members          | List, remove, role management, leave workspace                |
| Invites          | Email invites and shareable invite codes                       |
| Documents        | Upload, list, read, delete (with versioning and retention)    |
| Connections      | CRUD for encrypted provider credentials                       |
| Context Files    | CRUD for workspace detection context (multipart upload)       |
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
pipeline: upload, dispatch to runtime, detection, redaction, and result
storage. All tracing events use explicit, hierarchical target names following
the convention `<crate>::<module>::<submodule>` (e.g.,
`nvisy_server::handler::contexts`). This enables precise per-module log
filtering in production without relying on log levels alone.

### Health Checks

The server exposes a `/health` endpoint that returns per-component status
(PostgreSQL, NATS) with overall system health (healthy, degraded, unhealthy).
Cached health checks serve load balancer probes; authenticated requests
trigger real-time checks.

## Security Model

**Authentication** uses JWT tokens signed with Ed25519. Passwords are hashed
with Argon2.

**Authorization** is role-based, with permissions checked per workspace. Each
API operation requires a specific permission (e.g., manage connections, view
documents, upload files).

**Credential encryption** uses a two-tier key hierarchy. A master key derives
workspace-specific keys via HKDF-SHA256. Each workspace's connection credentials
are encrypted with XChaCha20-Poly1305 using its derived key. Encrypted data is
never exposed through the API.

See [Security](./SECURITY.md) for the full security model.
