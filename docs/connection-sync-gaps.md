# Connection Sync — Implementation Gaps

Tracks what is *not yet* implemented in the connection sync feature
(object-store import/export, scheduling, distributed worker). The happy path
— S3/Azure/GCS backends, import diff, manual + scheduled + distributed sync,
one-active-run enforcement, cursor pagination, and manual export — is complete.
The items below are edge-of-lifecycle gaps and doc/code mismatches.

Status legend: **done** = fixed · **tracked** = filed as an issue ·
**deferred** = intentionally out of scope, already self-documented.

## 1. Cursor / resumption was documented but never implemented — `done`

The schema and model docs claimed the encrypted `context` held resumption state
("last cursor, offset") that "each run reads and advances." No such logic
exists: `import_new` lists the source (`list("")`) and dedups against
already-imported keys via a DB lookup (`imported_keys_for_connection`), never a
cursor. Re-running a sync already picks up only new objects — that *is* the
incremental behavior — so the DB-diff design is correct and the docs were wrong.

Fixed: removed every mention of the cursor/offset "context" design and replaced
it with a description of the actual incremental key-diff behavior.

- `migrations/2026-01-19-045013_connections/up.sql` (credential blob + records_synced + table comments)
- `crates/nvisy-postgres/src/model/workspace_connection.rs`
- `crates/nvisy-postgres/src/model/workspace_connection_run.rs`
- Reality: `crates/nvisy-server/src/service/object/sync.rs:84-101`

## 2. Provider column comment overpromised — `done`

`provider` is free-form `TEXT`, but only `s3`, `azure`, and `gcs` are wired;
any other value returns "unknown object store provider" at connect time. The
comments advertised `openai, postgres, s3, pinecone`, which do not exist as
connectors.

Fixed: comments now list only the supported object-store providers.

- `migrations/2026-01-19-045013_connections/up.sql:115`
- `crates/nvisy-postgres/src/model/workspace_connection.rs` (provider field)
- `crates/nvisy-server/src/handler/request/connections.rs`, `.../response/connections.rs`
- Factory: `crates/nvisy-object/src/providers/mod.rs:74-87`

## 3. Sync cancellation — `tracked`

`SyncStatus::Cancelled` exists but is never set. There is no cancel endpoint,
and the transfer runs as a detached `tokio::spawn` with no stored handle, so an
in-flight run can only end by completing, failing, or hitting the 30-minute
`SYNC_TIMEOUT`. Highest-impact functional gap.

- Enum (unused): `crates/nvisy-postgres/src/types/enums/sync_status.rs:40`
- Routes (no cancel): `crates/nvisy-server/src/handler/connection_syncs.rs:239-256`
- Detached task: `crates/nvisy-server/src/handler/connection_syncs.rs:119-123`

## 4. Source-deletion handling — `tracked`

Import is strictly additive. When an object is deleted at the source, the
corresponding `workspace_file` is not removed or marked. The
`WebhookEvent::ConnectionDesynced` event is defined but never emitted.

- `crates/nvisy-server/src/service/object/sync.rs:96-113`

## 5. Webhook-triggered sync — `tracked`

`SyncTriggerType::Webhook` is a dead enum value: no inbound endpoint or consumer
ever constructs a webhook-triggered run. The webhook subsystem is outbound-only.

- Enum (unused): `crates/nvisy-postgres/src/types/enums/sync_trigger_type.rs:29`

## 6. Failure retry / backoff — `tracked`

A cleanly failed run is fail-and-forget. JetStream provides crash-redelivery
(job redelivered if the process dies before ack), but there is no retry or
backoff for business-level failures.

- `crates/nvisy-server/src/service/object/sync.rs:288-321`
- `crates/nvisy-server/src/service/object/worker.rs:197-206`

## 7. Scheduled export — `deferred`

Scheduling is import-only by design; enforced by a DB CHECK
(`schedule_cron IS NULL OR sync_mode = 'import'`) and documented at
`migrations/2026-01-19-045013_connections/up.sql:58-59`.

## 8. Bulk export — `deferred`

Export transfers one file per call; there is no `export_new` equivalent of the
import diff loop, and export does not record provenance rows.

## 9. Non-object-store connectors — `deferred`

`postgres`, `pinecone`, `openai` as *data sources* are not implemented. Only
object-store providers (s3/azure/gcs) exist. (The internal Postgres references
in the sync code are the app's own database, not a connector.)
