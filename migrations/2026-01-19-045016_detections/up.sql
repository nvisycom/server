-- Detections: one analysis pass of a file through a pipeline, its per-model
-- usage, and the transactional outbox that queues each analysis. A detection
-- can produce many redactions (see the redactions migration).

CREATE TYPE DETECTION_STATUS AS ENUM (
    'pending',      -- Enqueued for detection; no worker has picked it up yet
    'executing',    -- A worker is actively analyzing the document
    'complete',     -- Detection done; ready to redact
    'failed'        -- Detection failed with error
);

COMMENT ON TYPE DETECTION_STATUS IS 'Execution status of a detection: pending, executing, complete, or failed.';

-- How a detection was initiated.
CREATE TYPE PIPELINE_TRIGGER_TYPE AS ENUM (
    'user',         -- Started directly by a user
    'system'        -- Started automatically (e.g. a file upload auto-redacted)
);

COMMENT ON TYPE PIPELINE_TRIGGER_TYPE IS 'How a detection was initiated: by a user or by the system.';

-- The billing unit a model call metered. A call reports an open set of these
-- (tokens for LLMs, seconds for speech-to-text, pages/images for OCR, …); each
-- is one row keyed by this type, read together with its amount (never bare).
CREATE TYPE USAGE_METER AS ENUM (
    'tokens_input',     -- Input/prompt tokens (token-billed models)
    'tokens_output',    -- Output/completion tokens
    'tokens_total',     -- Total tokens as reported (not necessarily input + output)
    'seconds',          -- Seconds of media processed (speech-to-text)
    'pages',            -- Pages processed (document OCR, per page)
    'images',           -- Images processed (vision/OCR, per image)
    'characters'        -- Characters processed (per-character services)
);

COMMENT ON TYPE USAGE_METER IS 'The billing unit a model call metered: token counts, media seconds, pages, images, or characters.';

-- Detections table: one analysis pass of a file through a pipeline.
CREATE TABLE workspace_detections (
    -- Primary identifier
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References. A detection is workspace-scoped directly: an ad-hoc detection
    -- carries no pipeline, and deleting a pipeline detaches (SET NULL) its
    -- detections rather than erasing the workspace's analysis history.
    workspace_id    UUID                    NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    pipeline_id     UUID                    DEFAULT NULL REFERENCES workspace_pipelines (id) ON DELETE SET NULL,
    account_id      UUID                    NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,

    -- What a detection relates to. The input is the source document (required);
    -- the produced analysis (the "audit" findings set) is a blob pointed at by
    -- `audit_blob_id` below. The enrichment intermediate (OCR layout, transcript)
    -- is a blob the analysis may produce, served to the client; null until (and
    -- unless) an enricher runs. A detection is append-only history:
    -- the input cascades with a whole-workspace teardown; the intermediate blob
    -- reference clears (SET NULL) as the correct action for a produced artifact.
    input_document_id     UUID              NOT NULL REFERENCES workspace_documents (id) ON DELETE CASCADE,
    intermediate_blob_id  UUID              DEFAULT NULL REFERENCES workspace_blobs (id) ON DELETE SET NULL,

    -- The detection's base analysis: the engine's findings set (the "audit"),
    -- stored as bytes in a blob. NULL before the analysis commits and again once
    -- the bytes are reclaimed on retention (the detection row — a ledger record —
    -- is kept either way). Served as the detection's findings and reloaded to
    -- re-redact.
    audit_blob_id         UUID              DEFAULT NULL REFERENCES workspace_blobs (id) ON DELETE SET NULL,

    -- Detection attributes
    trigger_type    PIPELINE_TRIGGER_TYPE   NOT NULL DEFAULT 'user',
    status          DETECTION_STATUS        NOT NULL DEFAULT 'pending',

    -- Idempotency key from the initiating detect request; a repeat replays the
    -- existing detection instead of analyzing twice.
    idempotency_key TEXT                    DEFAULT NULL,
    CONSTRAINT workspace_detections_idempotency_key_length CHECK (idempotency_key IS NULL OR length(idempotency_key) BETWEEN 1 AND 255),

    -- The effective retention override this detection ran under, snapshotted at
    -- create time (from its pipeline, or supplied directly for an ad-hoc
    -- detection). Redaction reads it from here, so re-redacting stamps output
    -- retention from what the detection ran with rather than a since-edited
    -- pipeline. Null falls back to the workspace retention baseline.
    retention_override JSONB                DEFAULT NULL,

    -- Non-encrypted metadata for filtering and display. The engine's full
    -- per-call usage report (outcomes, durations, and the units each call spent)
    -- is kept here under `usage` for opaque drill-down; the structured per-call
    -- and per-meter rows for aggregation live in the workspace_detection_call and
    -- workspace_detection_call_meter tables below.
    metadata        JSONB                   NOT NULL DEFAULT '{}',
    CONSTRAINT workspace_detections_metadata_size CHECK (length(metadata::TEXT) BETWEEN 2 AND 65536),

    -- Accumulated reviewer edits (the engine's EditSet: suppress/retag/add) layered
    -- over the detection's immutable base analysis at redact time. Persisted here, not
    -- per redact request, so manual corrections survive between redactions and a solo
    -- user keeps their work without opening a review. NULL means no edits yet.
    edit_set        JSONB                   DEFAULT NULL,
    CONSTRAINT workspace_detections_edit_set_size CHECK (edit_set IS NULL OR length(edit_set::TEXT) <= 262144),

    -- Optimistic-concurrency version for edit_set, bumped on every edit write. An
    -- edit PATCH names the revision it read, and the write proceeds only if it still
    -- matches, so two reviewers editing the same detection cannot silently overwrite
    -- each other (a stale PATCH is rejected as a conflict).
    edit_set_rev    INTEGER                 NOT NULL DEFAULT 0,

    -- Detection lease: when a worker last claimed this detection. A redelivered
    -- job whose claim is still fresh is skipped (no double-analyze); a stale claim
    -- (a worker that died mid-analysis) can be re-claimed. Null until claimed.
    claimed_at      TIMESTAMPTZ             DEFAULT NULL,

    -- Timing
    started_at      TIMESTAMPTZ             NOT NULL DEFAULT current_timestamp,
    completed_at    TIMESTAMPTZ             DEFAULT NULL,
    CONSTRAINT workspace_detections_completed_after_started CHECK (completed_at IS NULL OR completed_at >= started_at),

    -- Soft-delete. A detection is a ledger record (it ran, it cost credits), so
    -- deleting one keeps the row and only releases its blob bytes; the row is hard-
    -- removed only when its workspace is purged.
    deleted_at      TIMESTAMPTZ             DEFAULT NULL,
    CONSTRAINT workspace_detections_deleted_after_started CHECK (deleted_at IS NULL OR deleted_at >= started_at)
);

-- A workspace's detections, newest first (the workspace detection list,
-- including ad-hoc detections with no pipeline).
CREATE INDEX workspace_detections_workspace_idx
    ON workspace_detections (workspace_id, started_at DESC)
    WHERE deleted_at IS NULL;

-- A pipeline's detections, newest first (the per-pipeline detection list).
CREATE INDEX workspace_detections_pipeline_idx
    ON workspace_detections (pipeline_id, started_at DESC)
    WHERE pipeline_id IS NOT NULL AND deleted_at IS NULL;

-- Detections triggered by an account, newest first.
CREATE INDEX workspace_detections_account_idx
    ON workspace_detections (account_id, started_at DESC)
    WHERE deleted_at IS NULL;

-- In-flight detections by status (queue and ready backlog).
CREATE INDEX workspace_detections_status_idx
    ON workspace_detections (status, started_at DESC)
    WHERE status IN ('pending', 'executing', 'complete');

-- Detections analyzing a given input document, newest first.
CREATE INDEX workspace_detections_input_document_idx
    ON workspace_detections (input_document_id, started_at DESC);

-- Back the enrichment intermediate blob reference, NULL until (and unless) a
-- detection's analysis runs an enricher (an analysis with none produces none).
CREATE INDEX workspace_detections_intermediate_blob_idx
    ON workspace_detections (intermediate_blob_id)
    WHERE intermediate_blob_id IS NOT NULL;

-- Back the base-audit blob reference, walked by the retention sweep that reclaims
-- the analysis bytes and nulls this pointer.
CREATE INDEX workspace_detections_audit_blob_idx
    ON workspace_detections (audit_blob_id)
    WHERE audit_blob_id IS NOT NULL;

-- Idempotent detect: at most one live detection per (workspace, idempotency key),
-- so a retry replays the existing detection whether or not it names a pipeline.
-- Partial on live rows: a soft-deleted detection does not block a fresh run under
-- the same key.
CREATE UNIQUE INDEX workspace_detections_idempotency_idx
    ON workspace_detections (workspace_id, idempotency_key)
    WHERE idempotency_key IS NOT NULL AND deleted_at IS NULL;

-- A detection is append-only history: its document reference is kept even after
-- the document is soft-deleted, so the record of what it analyzed survives; a
-- reference to a soft-deleted document resolves to "gone" at read time.

COMMENT ON TABLE workspace_detections IS 'Detections: one analysis pass of a document through a pipeline.';
COMMENT ON COLUMN workspace_detections.id IS 'Unique detection identifier';
COMMENT ON COLUMN workspace_detections.workspace_id IS 'Owning workspace';
COMMENT ON COLUMN workspace_detections.pipeline_id IS 'Pipeline whose config drove the detection; NULL for an ad-hoc detection or after its pipeline was deleted';
COMMENT ON COLUMN workspace_detections.account_id IS 'Account that triggered the detection';
COMMENT ON COLUMN workspace_detections.input_document_id IS 'Source document the detection analyzes';
COMMENT ON COLUMN workspace_detections.intermediate_blob_id IS 'Enrichment intermediate blob (OCR layout, transcript) served to the client; NULL if no enricher ran';
COMMENT ON COLUMN workspace_detections.audit_blob_id IS 'Base analysis (findings set) blob; NULL before analysis commits and once the bytes are reclaimed on retention (the row is kept)';
COMMENT ON COLUMN workspace_detections.trigger_type IS 'How the detection was initiated';
COMMENT ON COLUMN workspace_detections.status IS 'Current detection status';
COMMENT ON COLUMN workspace_detections.idempotency_key IS 'Detect idempotency key (dedupes retries)';
COMMENT ON COLUMN workspace_detections.retention_override IS 'Retention override the detection ran under, snapshotted at create; read on redact so output retention reflects the run, not a since-edited pipeline. NULL falls back to the workspace baseline';
COMMENT ON COLUMN workspace_detections.metadata IS 'Non-encrypted metadata for filtering/display; holds the full per-call usage report under `usage`';
COMMENT ON COLUMN workspace_detections.edit_set IS 'Accumulated reviewer edits (EditSet) applied over the base analysis at redact time; persists across redactions. NULL means none';
COMMENT ON COLUMN workspace_detections.edit_set_rev IS 'Optimistic-concurrency version of edit_set, bumped on each edit write; an edit PATCH must name the revision it read';
COMMENT ON COLUMN workspace_detections.claimed_at IS 'Detection lease: when a worker last claimed this detection';
COMMENT ON COLUMN workspace_detections.started_at IS 'When the detection started';
COMMENT ON COLUMN workspace_detections.completed_at IS 'When the detection completed; NULL while in flight';
COMMENT ON COLUMN workspace_detections.deleted_at IS 'Soft-deletion timestamp; NULL means live. The row is kept as a ledger record; only its blob bytes are reclaimed';

-- Inference usage for a detection, at the grain the engine reports it: one row
-- per model *call*. A detection's recognizers may call the same model more than
-- once (and the same model may be called with different outcomes), so calls are
-- NOT collapsed by model here — each call keeps its own outcome and duration.
-- Per-model and per-meter rollups are produced by aggregating these at read
-- time. The full per-recognizer report is also kept on the detection
-- (metadata.usage) for opaque drill-down.
CREATE TABLE workspace_detection_call (
    -- Primary identifier
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- The detection this call belongs to; calls are deleted with their detection.
    detection_id    UUID                    NOT NULL REFERENCES workspace_detections (id) ON DELETE CASCADE,

    -- The model called and its optional version. Not unique per detection: the
    -- same (model, version) may be called several times, each its own row.
    model           TEXT                    NOT NULL,
    version         TEXT                    DEFAULT NULL,
    CONSTRAINT workspace_detection_call_model_length CHECK (length(model) BETWEEN 1 AND 255),
    CONSTRAINT workspace_detection_call_version_length CHECK (version IS NULL OR length(version) BETWEEN 1 AND 255),

    -- How the call ended: 'ok', or the engine's error-kind label. An open set, so
    -- stored as text rather than an enum.
    outcome         TEXT                    NOT NULL,
    CONSTRAINT workspace_detection_call_outcome_length CHECK (length(outcome) BETWEEN 1 AND 64),

    -- This call's wall-clock time (the model round-trip), in milliseconds.
    -- Distinct from the detection's total duration, which spans every call plus
    -- decode and enrichment.
    duration_ms     BIGINT                  NOT NULL,
    CONSTRAINT workspace_detection_call_duration_non_negative CHECK (duration_ms >= 0),

    -- What the call produced (entities, segments, …), when the response reports a
    -- count; NULL on failure or a response with no meaningful count.
    output_count    BIGINT                  DEFAULT NULL,
    CONSTRAINT workspace_detection_call_output_count_non_negative CHECK (output_count IS NULL OR output_count >= 0)
);

-- Per-detection drill-down and the join key for the meter rollups.
CREATE INDEX workspace_detection_call_detection_idx
    ON workspace_detection_call (detection_id);

-- Usage rollups by model across detections.
CREATE INDEX workspace_detection_call_model_idx
    ON workspace_detection_call (model);

COMMENT ON TABLE workspace_detection_call IS 'One model call made during a detection, with its outcome and duration.';
COMMENT ON COLUMN workspace_detection_call.id IS 'Unique call row identifier';
COMMENT ON COLUMN workspace_detection_call.detection_id IS 'Detection this call belongs to';
COMMENT ON COLUMN workspace_detection_call.model IS 'Model identifier the call used';
COMMENT ON COLUMN workspace_detection_call.version IS 'Model version, if the provider reported one';
COMMENT ON COLUMN workspace_detection_call.outcome IS 'How the call ended: ok, or the engine error-kind label';
COMMENT ON COLUMN workspace_detection_call.duration_ms IS 'This call''s wall-clock round-trip, in milliseconds';
COMMENT ON COLUMN workspace_detection_call.output_count IS 'Items the call produced, if the response reported a count; NULL otherwise';

-- The billing meters a call spent: one row per (call, meter). A call that billed
-- nothing (e.g. a failed call) has no rows here. `amount` is NUMERIC because a
-- meter may be fractional (seconds) or an integer count (tokens, pages, …); it
-- is only meaningful read together with `meter`.
CREATE TABLE workspace_detection_call_meter (
    -- Primary identifier
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- The call this meter belongs to; meters are deleted with their call.
    call_id         UUID                    NOT NULL REFERENCES workspace_detection_call (id) ON DELETE CASCADE,

    -- Which unit was metered, and how much. The amount is non-negative and read
    -- only in the context of the meter (1500 means tokens or seconds or pages
    -- depending on this column).
    meter           USAGE_METER             NOT NULL,
    amount          NUMERIC                 NOT NULL,
    CONSTRAINT workspace_detection_call_meter_amount_non_negative CHECK (amount >= 0)
);

-- One amount per meter per call; a call reports each unit at most once.
CREATE UNIQUE INDEX workspace_detection_call_meter_call_meter_key
    ON workspace_detection_call_meter (call_id, meter);

-- Cross-detection rollups grouped by meter.
CREATE INDEX workspace_detection_call_meter_meter_idx
    ON workspace_detection_call_meter (meter);

COMMENT ON TABLE workspace_detection_call_meter IS 'A billing meter (tokens, seconds, pages, …) a model call spent.';
COMMENT ON COLUMN workspace_detection_call_meter.id IS 'Unique meter row identifier';
COMMENT ON COLUMN workspace_detection_call_meter.call_id IS 'Call this meter belongs to';
COMMENT ON COLUMN workspace_detection_call_meter.meter IS 'Which unit was metered';
COMMENT ON COLUMN workspace_detection_call_meter.amount IS 'How much of the unit was spent; read only together with meter';

-- Detection-job outbox: the transactional-outbox queue for detection analysis.
-- Creating a detection inserts one row here in the same transaction as the
-- detection, so the two commit or roll back together; a background drainer then
-- publishes each pending row onto the detection NATS work-queue. This removes the
-- dual-write between the detection row and the queue: an analysis is never lost
-- to an enqueue that failed after the row committed, nor is a detection ever
-- marked failed for an enqueue that in fact went through.
CREATE TABLE workspace_detection_jobs (
    -- Primary identifier
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- The detection this job analyzes; the row is deleted with its detection.
    detection_id    UUID          NOT NULL REFERENCES workspace_detections (id) ON DELETE CASCADE,

    -- The job. A serialized `DetectionJob`: the workspace, detection, and the
    -- optional per-request scope the drainer publishes to the worker.
    job             JSONB         NOT NULL,
    CONSTRAINT workspace_detection_jobs_job_size CHECK (length(job::TEXT) BETWEEN 2 AND 16384),

    -- Drainer bookkeeping: the row's processing state, how many publish attempts
    -- it has taken, and the earliest time it may next be claimed (advanced by a
    -- backoff on each failed attempt so a failing row does not spin at the head of
    -- the queue).
    status          OUTBOX_STATUS NOT NULL DEFAULT 'pending',
    attempts        INTEGER       NOT NULL DEFAULT 0,
    CONSTRAINT workspace_detection_jobs_attempts_non_negative CHECK (attempts >= 0),
    next_attempt_at TIMESTAMPTZ   NOT NULL DEFAULT current_timestamp,

    -- Lifecycle timestamps
    created_at      TIMESTAMPTZ   NOT NULL DEFAULT current_timestamp,

    -- When a terminal row (processed or failed) was resolved by an operator; NULL
    -- until then. A manual affordance for inspecting the outbox after the fact.
    resolved_at     TIMESTAMPTZ   DEFAULT NULL,
    CONSTRAINT workspace_detection_jobs_resolved_only_when_terminal
        CHECK (resolved_at IS NULL OR status IN ('processed', 'failed')),
    CONSTRAINT workspace_detection_jobs_resolved_after_created
        CHECK (resolved_at IS NULL OR resolved_at >= created_at)
);

-- The drainer's claim queue: pending rows ordered by due time then age, so a
-- batch claims the oldest due rows. Partial so it stays small as processed and
-- failed rows accumulate.
CREATE INDEX workspace_detection_jobs_pending_idx
    ON workspace_detection_jobs (next_attempt_at, created_at)
    WHERE status = 'pending';

-- Back the detection foreign key so a detection delete cascades without scanning
-- the whole outbox (Postgres does not index a referencing column automatically,
-- and the partial claim index above does not cover it).
CREATE INDEX workspace_detection_jobs_detection_idx
    ON workspace_detection_jobs (detection_id);

COMMENT ON TABLE workspace_detection_jobs IS 'Transactional outbox of detection jobs, drained to the detection NATS work-queue.';
COMMENT ON COLUMN workspace_detection_jobs.id IS 'Unique outbox row identifier';
COMMENT ON COLUMN workspace_detection_jobs.detection_id IS 'Detection this job analyzes';
COMMENT ON COLUMN workspace_detection_jobs.job IS 'Serialized DetectionJob published to the worker (JSON, 2B-16KB)';
COMMENT ON COLUMN workspace_detection_jobs.status IS 'Processing state: pending, processed, or failed (dead-lettered)';
COMMENT ON COLUMN workspace_detection_jobs.attempts IS 'Number of publish attempts the drainer has made';
COMMENT ON COLUMN workspace_detection_jobs.next_attempt_at IS 'Earliest time the row may next be claimed; advanced by a backoff after each failed attempt';
COMMENT ON COLUMN workspace_detection_jobs.created_at IS 'Timestamp when the job was queued';
COMMENT ON COLUMN workspace_detection_jobs.resolved_at IS 'When a terminal (processed or failed) row was resolved by an operator; NULL until then. A manual affordance for inspecting the outbox after the fact';

-- Detection policy-version pin: the exact policy versions a detection's analysis
-- ran against, captured when the analysis is committed. A policy's definition is
-- versioned (see the policies migration) and consumed live at run time; recording
-- the resolved versions here makes a detection reproducible against the config
-- that produced it, independent of later policy edits or soft-deletes. A pinned
-- version can never be deleted (ON DELETE RESTRICT); versions are immutable anyway.
CREATE TABLE workspace_detection_policy_versions (
    -- The detection whose analysis pinned these versions; the pin is deleted with
    -- its detection.
    detection_id        UUID        NOT NULL REFERENCES workspace_detections (id) ON DELETE CASCADE,

    -- The policy version the analysis consumed. A pinned version can never be
    -- deleted (versions are immutable anyway). The worker only pins versions it
    -- resolved within the run's workspace, so a foreign version is never recorded.
    policy_version_id   UUID        NOT NULL REFERENCES workspace_policy_versions (id) ON DELETE RESTRICT,

    PRIMARY KEY (detection_id, policy_version_id)
);

-- Back the policy-version foreign key (so a version delete attempt is checked
-- without scanning, and per-version lookups are indexed).
CREATE INDEX workspace_detection_policy_versions_version_idx
    ON workspace_detection_policy_versions (policy_version_id);

COMMENT ON TABLE workspace_detection_policy_versions IS 'The exact policy versions a detection''s analysis ran against; pins a detection to reproducible config.';
COMMENT ON COLUMN workspace_detection_policy_versions.detection_id IS 'Detection whose analysis pinned these versions';
COMMENT ON COLUMN workspace_detection_policy_versions.policy_version_id IS 'Policy version the analysis consumed';

-- Detection run events feed the activity log, webhooks, and (for terminal
-- completion/failure) in-app notifications.
ALTER TYPE ACTIVITY_TYPE ADD VALUE IF NOT EXISTS 'pipeline.detection.started';
ALTER TYPE ACTIVITY_TYPE ADD VALUE IF NOT EXISTS 'pipeline.detection.completed';
ALTER TYPE ACTIVITY_TYPE ADD VALUE IF NOT EXISTS 'pipeline.detection.failed';

ALTER TYPE WEBHOOK_EVENT ADD VALUE IF NOT EXISTS 'pipeline.detection.started';
ALTER TYPE WEBHOOK_EVENT ADD VALUE IF NOT EXISTS 'pipeline.detection.completed';
ALTER TYPE WEBHOOK_EVENT ADD VALUE IF NOT EXISTS 'pipeline.detection.failed';

ALTER TYPE NOTIFICATION_EVENT ADD VALUE IF NOT EXISTS 'pipeline.detection.completed';
ALTER TYPE NOTIFICATION_EVENT ADD VALUE IF NOT EXISTS 'pipeline.detection.failed';
