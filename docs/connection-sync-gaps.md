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

## 3. Sync cancellation — `done`

Added `POST /connections/{id}/syncs/{syncId}/cancel/`. The run transitions to
`Cancelled` only from an active state (a finished run returns 409); the existing
`cancel_workspace_connection_run` query is now status-guarded and returns
`Option`. The background transfer is not force-aborted, but its completion is
status-guarded (`complete`/`fail` only transition from active states) so a
cancelled run is never overwritten, and the 30-minute timeout bounds the work.

- `crates/nvisy-server/src/handler/connection_syncs.rs` (cancel handler + route)
- `crates/nvisy-postgres/src/query/workspace_connection_run.rs` (status-guarded cancel)

## 4. Source-deletion handling — `done`

Per-connection `deletion_policy` (`SYNC_DELETION_POLICY` enum) controls what an
import does when a previously-imported source object is gone: `ignore` (default,
additive-only), `soft_delete` (mark the file deleted + emit `ConnectionDesynced`),
or `hard_delete` (also remove the stored NATS object). Reconciliation reuses the
single source listing and tolerates per-file failures; a desync event fires once
per run when anything was removed.

- `crates/nvisy-postgres/src/types/enums/sync_deletion_policy.rs`
- `crates/nvisy-server/src/service/object/sync.rs` (`reconcile_deletions`)
- `crates/nvisy-postgres/src/query/workspace_file.rs` (`imported_files_for_connection`, `hard_delete_workspace_file`)

## 5. Webhook-triggered sync — `tracked`

`SyncTriggerType::Webhook` is a dead enum value: no inbound endpoint or consumer
ever constructs a webhook-triggered run. The webhook subsystem is outbound-only.
Filed as [#175](https://github.com/nvisycom/server/issues/175) — low priority
since manual sync via the API already covers the use case.

- Enum (unused): `crates/nvisy-postgres/src/types/enums/sync_trigger_type.rs:29`

## 6. Failure retry / backoff — `done`

Scheduled runs carry a 1-based `attempt`. On a failed scheduled run the worker
re-enqueues the job with `attempt + 1`, up to a bounded maximum, after a linear
backoff that runs in a detached task so the consumer is not blocked. Manual runs
are not retried (the caller can re-trigger). Crash-redelivery via JetStream is
unchanged and independent.

- `crates/nvisy-server/src/service/object/worker.rs` (`maybe_retry`, job `attempt`)

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
