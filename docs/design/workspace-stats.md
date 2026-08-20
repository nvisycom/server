# Workspace stats — design

Status: **Tier 1 in progress** · Tiers 2–3 specced, gated on an upstream elide change (Tier 3).

## Goal

Surface the stats that reflect **what the redaction engine is actually doing and
costing** for a workspace — run health, storage, detections, and (once elide
exposes it) token/inference cost. Not vanity counts (member-role breakdowns,
per-status invite tallies); those are derivable on demand and nobody needs them
aggregated.

Every stat here has a named consumer: a **workspace stats endpoint**
(`GET /workspaces/{ws}/stats`) that backs a dashboard header. No object is added
without that consumer — the deleted `*_summary` / `*_history` views were the
anti-pattern (schema with no reader) and are not reintroduced.

## Tiers

The stats split by **data readiness**, not by importance:

| Tier | Stats | Data source | Blocked on |
|------|-------|-------------|------------|
| **1** | run health, storage | existing columns | nothing — ship now |
| **2** | detection counts / entity-label breakdown | elide `Audit` (already returned; not persisted) | a schema addition + a persist step |
| **3** | tokens spent, inference cost | **does not exist in elide** | an upstream elide `Usage` type |

---

## Tier 1 — run health + storage (ready now)

`workspace_files` carries `workspace_id` directly, so its aggregates are
single-table. `workspace_pipeline_runs` does **not** — a run is scoped to a
workspace through `pipeline_id → workspace_pipelines.workspace_id`, so run-health
aggregates join runs to their pipeline and filter on the pipeline's
`workspace_id`. The existing `workspace_pipeline_runs_pipeline_idx (pipeline_id,
started_at DESC)` supports that join.

### Storage (`workspace_files`, live rows only)

| Stat | Formula |
|------|---------|
| `total_bytes` | `SUM(file_size_bytes)` |
| `file_count` | `COUNT(*)` |
| `by_kind` | `COUNT(*), SUM(file_size_bytes) GROUP BY file_kind` → `{original, redacted, audit}` |

`file_kind` values: `original`, `redacted`, `audit`.

**Storage semantics — live logical bytes.** `total_bytes` counts live rows
(`deleted_at IS NULL`), matching the existing `get_account_storage_usage`
(`query/workspace_file.rs`) — already the codebase's definition of "storage
used". It answers "how much live data does this workspace hold", and is the right
number for a usage stat. With the manual-delete bug below fixed, live logical
bytes and physical NATS bytes stay in sync, so no second number is needed.

**Bug to fix first: manual file delete leaks its NATS object.** The retention
worker's `expire_file` (`file_retention.rs`) tears a file down correctly in three
steps: `clear_run_file_references` → `delete_workspace_file` (soft-delete) →
`delete_object` (purge NATS). The manual `delete_file` handler
(`handler/files.rs`) does **only** the middle step — it soft-deletes the row but
never purges the object *and* never clears run references. Consequences:

1. **Storage leak** — the NATS object is orphaned forever (the retention sweep
   only touches `expires_at < now`, which a manual delete does not set).
2. **Dangling references** — a run's `audit_file_id`/`output_file_id` FKs are
   `ON DELETE SET NULL`, which fires only on a *hard* delete; the soft delete
   leaves them pointing at a tombstoned file.

Fix: the manual delete must run the same teardown as expiry. Factor the
`clear_run_file_references` + soft-delete + `delete_object` sequence into a shared
path (e.g. lift `expire_file`'s body into a reusable `purge_file`) and call it
from both the retention worker and `delete_file`. This is a prerequisite for the
storage stat to be meaningful, and a real correctness fix regardless of stats.

### Run health (`workspace_pipeline_runs`)

| Stat | Formula |
|------|---------|
| `runs_total` | `COUNT(*)` (optionally windowed: `started_at >= now() - $window`) |
| `by_status` | `COUNT(*) GROUP BY status` → `{queued, analyzing, analyzed, completed, failed, cancelled}` |
| `error_rate` | `failed / (completed + failed)` — terminal runs only |
| `avg_duration_s` | `AVG(EXTRACT(EPOCH FROM (completed_at - started_at)))` where `completed_at IS NOT NULL` |
| `p95_duration_s` | `percentile_cont(0.95) WITHIN GROUP (ORDER BY completed_at - started_at)` |

`status` values: `queued`, `analyzing`, `analyzed` (active); `completed`,
`failed`, `cancelled` (terminal).

### Delivery — live Diesel queries, no new DB objects

At current scale these are cheap workspace-scoped GROUP BYs; a view or matview
would be premature. The endpoint issues two aggregate queries (files, runs) and
assembles the response. Revisit (→ matview or a counter table) only when a
measured query crosses a latency budget.

### Index gap (the one real perf finding)

`workspace_pipeline_runs_status_idx` is **partial on active statuses only**:

```sql
CREATE INDEX workspace_pipeline_runs_status_idx
    ON workspace_pipeline_runs (status, started_at DESC)
    WHERE status IN ('queued', 'analyzing', 'analyzed');
```

The run-health aggregates read **terminal** runs (`completed`/`failed`, and
durations over `completed_at`), which this partial index does not serve. Runs
have no `workspace_id`, so the aggregate joins on `pipeline_id`; the existing
`workspace_pipeline_runs_pipeline_idx (pipeline_id, started_at DESC)` covers the
join but not a status breakdown. If profiling shows the status/duration grouping
is hot, add:

```sql
-- Backs run-health aggregates (counts by status, durations) per pipeline.
CREATE INDEX workspace_pipeline_runs_status_stats_idx
    ON workspace_pipeline_runs (pipeline_id, status);
```

Edited into `2026-01-19-045014_pipelines/up.sql` (pre-1.0, edit-in-place) with a
matching `down.sql` drop, then the DB is reset. Defer it until the query is
written and measured — the pipeline_idx join may already suffice.

---

## Tier 2 — persist detection metadata (no elide change)

elide's `analyze()` already returns an `Audit` carrying the detected entities
(`body` + per-part `EntityGroup`s, each entity labeled). Today we store the audit
*blob* as an encrypted file but extract **nothing queryable** from it, so we can't
answer "how many detections, of what kinds, over time".

**Add to the run record** (queryable, not free-form): a detection count and a
per-label rollup, populated in the detection worker right after `analyze()`
(`service/detection/worker.rs`, where `analyzed` is in hand).

- `RunMetadata` (the typed `workspace_pipeline_runs.metadata` JSONB) gains:
  `detection_count: u32`, `entities_by_label: BTreeMap<String, u32>`.
- Or, if we want to aggregate across runs efficiently, a dedicated
  `detection_count BIGINT` column + a partial index — decided when Tier 2 lands.

**Stats unlocked:** total detections per workspace/pipeline, top entity types
found, detections over time.

---

## Tier 3 — token / inference cost (needs upstream elide)

**Gap confirmed:** there is no `Usage`/`TokenUsage`/cost type anywhere in the
elide crates. LLM-backed detection spends tokens, but `analyze()`'s `Audit` does
not surface them. This is the stat the product most wants ("how much are we
spending") and it cannot be built until elide emits the data.

### Upstream elide change (done before this PR merges)

elide's analyze result must carry per-run inference usage. Proposed shape:

```rust
// elide-pipeline: attached to Audit (or a sibling result the analyze call returns)
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub model: String,     // which model produced the detections
    // optionally: per-provider cost inputs, or leave costing to the caller
}
```

`Audit` (or the analyze return) gains `usage: Option<Usage>` — `Option` because a
non-LLM (deterministic) detection pass spends no tokens.

### Server side (this PR, after the elide bump)

Persist usage on the run and aggregate it:

- Run record gains `input_tokens BIGINT`, `output_tokens BIGINT`, `model TEXT`
  (nullable — deterministic runs have none), written in the worker from
  `analyzed.usage`.
- Cost is derived at read time from a model→price table (kept in server config,
  not the DB, so prices change without a migration).

**Stats unlocked:** tokens in/out per workspace/pipeline, estimated cost, cost
over time, cost per redacted document.

---

## Build order

1. **Tier 1** — the stats endpoint + the `_stats_idx` index. Ships now.
2. **Tier 3 schema** — after the elide bump lands the `Usage` type: run columns +
   worker persist + cost-at-read.
3. **Tier 2** — detection-count persistence + rollup.

All three land in this PR; Tier 3 is unblocked by the upstream elide change the
author applies before merge.
