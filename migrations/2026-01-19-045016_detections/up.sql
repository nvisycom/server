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

-- Detections table: one analysis pass of a file through a pipeline.
CREATE TABLE workspace_detections (
    -- Primary identifier
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References
    pipeline_id     UUID                    NOT NULL REFERENCES workspace_pipelines (id) ON DELETE CASCADE,
    account_id      UUID                    NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,

    -- The two files a detection relates to, each a distinct role. The input is
    -- the source document (required); the audit blob is produced by the analysis,
    -- so it is null until the pass completes. Redacted outputs are not here — a
    -- detection produces many redactions, each owning its own output (see the
    -- workspace_redactions table below).
    --   input:  the original document being analyzed.
    --   audit:  the engine's analysis (Audit), a `file_kind = audit` file held
    --           between detect and redact; redact reads it as the source of truth.
    -- A detection is append-only audit history: its file references survive the
    -- files themselves. Files are only ever soft-deleted (their objects purged),
    -- so these ON DELETE actions fire only on a hard delete. The app never hard-
    -- deletes an individual file; the one hard delete is a whole-workspace
    -- teardown, which cascades files and detections away together. The input
    -- therefore cascades — a workspace deletion that removes the source document
    -- should remove its detections too. The produced audit uses ON DELETE SET
    -- NULL so that, in that same teardown, it clears rather than cascades
    -- (redundant here, but the correct action for a produced artifact).
    input_file_id   UUID                    NOT NULL REFERENCES workspace_files (id) ON DELETE CASCADE,
    audit_file_id   UUID                    DEFAULT NULL REFERENCES workspace_files (id) ON DELETE SET NULL,
    intermediates_file_id UUID              DEFAULT NULL REFERENCES workspace_files (id) ON DELETE SET NULL,

    -- Detection attributes
    trigger_type    PIPELINE_TRIGGER_TYPE   NOT NULL DEFAULT 'user',
    status          DETECTION_STATUS        NOT NULL DEFAULT 'pending',

    -- Idempotency key from the initiating detect request; a repeat replays the
    -- existing detection instead of analyzing twice.
    idempotency_key TEXT                    DEFAULT NULL,
    CONSTRAINT workspace_detections_idempotency_key_length CHECK (idempotency_key IS NULL OR length(idempotency_key) BETWEEN 1 AND 255),

    -- Non-encrypted metadata for filtering and display. The engine's full
    -- per-recognizer usage report (durations, per-model token counts) is kept
    -- here under `usage` for drill-down; per-model token totals for usage
    -- aggregation live in the workspace_detection_usage table below.
    metadata        JSONB                   NOT NULL DEFAULT '{}',
    CONSTRAINT workspace_detections_metadata_size CHECK (length(metadata::TEXT) BETWEEN 2 AND 65536),

    -- Detection lease: when a worker last claimed this detection. A redelivered
    -- job whose claim is still fresh is skipped (no double-analyze); a stale claim
    -- (a worker that died mid-analysis) can be re-claimed. Null until claimed.
    claimed_at      TIMESTAMPTZ             DEFAULT NULL,

    -- Timing
    started_at      TIMESTAMPTZ             NOT NULL DEFAULT current_timestamp,
    completed_at    TIMESTAMPTZ             DEFAULT NULL,
    CONSTRAINT workspace_detections_completed_after_started CHECK (completed_at IS NULL OR completed_at >= started_at)
);

-- A pipeline's detections, newest first (the detection list).
CREATE INDEX workspace_detections_pipeline_idx
    ON workspace_detections (pipeline_id, started_at DESC);

-- Detections triggered by an account, newest first.
CREATE INDEX workspace_detections_account_idx
    ON workspace_detections (account_id, started_at DESC);

-- In-flight detections by status (queue and ready backlog).
CREATE INDEX workspace_detections_status_idx
    ON workspace_detections (status, started_at DESC)
    WHERE status IN ('pending', 'executing', 'complete');

-- Detections analyzing a given input file, newest first.
CREATE INDEX workspace_detections_input_file_idx
    ON workspace_detections (input_file_id, started_at DESC);

-- The file-expiry sweep's hold check matches a candidate file against a
-- detection's input OR audit file; the input side is covered above, this covers
-- the audit side. Partial, since most detections eventually carry an audit but
-- the column is NULL until analysis writes it.
CREATE INDEX workspace_detections_audit_file_idx
    ON workspace_detections (audit_file_id)
    WHERE audit_file_id IS NOT NULL;

-- Same shape for the enrichment intermediates file, NULL until (and unless) a
-- detection's analysis runs an enricher (an analysis with none produces no file).
CREATE INDEX workspace_detections_intermediates_file_idx
    ON workspace_detections (intermediates_file_id)
    WHERE intermediates_file_id IS NOT NULL;

-- Idempotent detect: at most one detection per (pipeline, idempotency key).
CREATE UNIQUE INDEX workspace_detections_idempotency_idx
    ON workspace_detections (pipeline_id, idempotency_key)
    WHERE idempotency_key IS NOT NULL;

-- A detection is append-only history: its file references (input/audit) are kept
-- even after those files are deleted, so the record of what it analyzed
-- survives. The `ON DELETE SET NULL` FK would fire only on a hard file delete,
-- which never happens (files are soft-deleted); a reference to a soft-deleted
-- file resolves to "gone" at read time, distinct from a NULL that means the
-- detection never had one.

COMMENT ON TABLE workspace_detections IS 'Detections: one analysis pass of a file through a pipeline.';
COMMENT ON COLUMN workspace_detections.id IS 'Unique detection identifier';
COMMENT ON COLUMN workspace_detections.pipeline_id IS 'Pipeline whose config drove the detection';
COMMENT ON COLUMN workspace_detections.account_id IS 'Account that triggered the detection';
COMMENT ON COLUMN workspace_detections.input_file_id IS 'Source document the detection analyzes';
COMMENT ON COLUMN workspace_detections.audit_file_id IS 'Audit file (file_kind=audit) holding the analysis between detect and redact';
COMMENT ON COLUMN workspace_detections.intermediates_file_id IS 'Intermediates file (file_kind=intermediate) holding the enrichment (OCR layout, transcript) served to the client';
COMMENT ON COLUMN workspace_detections.trigger_type IS 'How the detection was initiated';
COMMENT ON COLUMN workspace_detections.status IS 'Current detection status';
COMMENT ON COLUMN workspace_detections.idempotency_key IS 'Detect idempotency key (dedupes retries)';
COMMENT ON COLUMN workspace_detections.metadata IS 'Non-encrypted metadata for filtering/display; holds the full per-recognizer usage report under `usage`';
COMMENT ON COLUMN workspace_detections.claimed_at IS 'Detection lease: when a worker last claimed this detection';
COMMENT ON COLUMN workspace_detections.started_at IS 'When the detection started';
COMMENT ON COLUMN workspace_detections.completed_at IS 'When the detection completed; NULL while in flight';

-- Per-model inference usage for a detection: one row per distinct model a
-- detection's recognizers used. Token counts are aggregated across the
-- recognizers that shared a model, letting usage analytics report tokens broken
-- down by model (a detection's summed tokens cannot, since it may mix models).
-- The full per-recognizer report is kept on the detection (metadata.usage) for
-- drill-down; this table is the aggregation surface.
CREATE TABLE workspace_detection_usage (
    -- Primary identifier
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- The detection this usage belongs to; usage is deleted with its detection.
    detection_id    UUID                    NOT NULL REFERENCES workspace_detections (id) ON DELETE CASCADE,

    -- The model and its optional version. Identity is (detection, model,
    -- version): a detection may use the same model at more than one version, and
    -- each is its own row (uniqueness enforced by an index below that normalizes
    -- a NULL version).
    model           TEXT                    NOT NULL,
    version         TEXT                    DEFAULT NULL,
    CONSTRAINT workspace_detection_usage_model_length CHECK (length(model) BETWEEN 1 AND 255),
    CONSTRAINT workspace_detection_usage_version_length CHECK (version IS NULL OR length(version) BETWEEN 1 AND 255),

    -- Token counts as the provider reported them. Each is independently nullable:
    -- `total` is NOT necessarily input + output (a provider may report only a
    -- total, or a total that also counts cached/reasoning tokens), so all three
    -- are carried faithfully and never derived from one another.
    input_tokens    BIGINT                  DEFAULT NULL,
    output_tokens   BIGINT                  DEFAULT NULL,
    total_tokens    BIGINT                  DEFAULT NULL,
    CONSTRAINT workspace_detection_usage_input_non_negative CHECK (input_tokens IS NULL OR input_tokens >= 0),
    CONSTRAINT workspace_detection_usage_output_non_negative CHECK (output_tokens IS NULL OR output_tokens >= 0),
    CONSTRAINT workspace_detection_usage_total_non_negative CHECK (total_tokens IS NULL OR total_tokens >= 0),

    -- Wall-clock time this model's recognizers spent, in milliseconds.
    duration_ms     BIGINT                  NOT NULL DEFAULT 0,
    CONSTRAINT workspace_detection_usage_duration_non_negative CHECK (duration_ms >= 0)
);

-- One row per (detection, model, version); a NULL version is normalized so two
-- unversioned rows for the same model collide instead of both being inserted.
-- Leads with detection_id, so it also serves per-detection drill-down lookups.
CREATE UNIQUE INDEX workspace_detection_usage_detection_model_version_key
    ON workspace_detection_usage (detection_id, model, COALESCE(version, ''));

-- Usage rollups by model across detections.
CREATE INDEX workspace_detection_usage_model_idx
    ON workspace_detection_usage (model);

COMMENT ON TABLE workspace_detection_usage IS 'Per-model inference token usage for a detection.';
COMMENT ON COLUMN workspace_detection_usage.id IS 'Unique usage row identifier';
COMMENT ON COLUMN workspace_detection_usage.detection_id IS 'Detection this usage belongs to';
COMMENT ON COLUMN workspace_detection_usage.model IS 'Model identifier the recognizers used';
COMMENT ON COLUMN workspace_detection_usage.version IS 'Model version, if the provider reported one';
COMMENT ON COLUMN workspace_detection_usage.input_tokens IS 'Input/prompt tokens for this model; NULL if not reported';
COMMENT ON COLUMN workspace_detection_usage.output_tokens IS 'Output/completion tokens for this model; NULL if not reported';
COMMENT ON COLUMN workspace_detection_usage.total_tokens IS 'Total tokens as reported (not necessarily input + output); NULL if not reported';
COMMENT ON COLUMN workspace_detection_usage.duration_ms IS 'Wall-clock time this model spent, in milliseconds';

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
