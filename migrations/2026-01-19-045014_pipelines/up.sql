-- Pipelines: redaction pipeline definitions, their runs, and produced files.
-- A pipeline is a workspace-scoped detection/redaction config; a run is one
-- pass of a file through that config. Policy references live in a join table
-- declared alongside policies, not embedded here.

-- Lifecycle status of a pipeline definition.
CREATE TYPE PIPELINE_STATUS AS ENUM (
    'draft',        -- Pipeline is being configured
    'enabled',      -- Pipeline is ready to run
    'disabled'      -- Pipeline is turned off
);

COMMENT ON TYPE PIPELINE_STATUS IS 'Lifecycle status of a pipeline definition: draft, enabled, or disabled.';

-- Execution status of a pipeline run.
CREATE TYPE PIPELINE_RUN_STATUS AS ENUM (
    'queued',       -- Enqueued for detection; no worker has picked it up yet
    'analyzing',    -- A worker is actively analyzing the document
    'analyzed',     -- Detection done; awaiting reviewer verification
    'completed',    -- Redaction applied; run finished
    'failed',       -- Run failed with error
    'cancelled'     -- Run was cancelled by user
);

COMMENT ON TYPE PIPELINE_RUN_STATUS IS 'Execution status of a pipeline run.';

-- How a pipeline run was initiated.
CREATE TYPE PIPELINE_TRIGGER_TYPE AS ENUM (
    'user',         -- Started directly by a user
    'system'        -- Started automatically (e.g. a file upload auto-redacted)
);

COMMENT ON TYPE PIPELINE_TRIGGER_TYPE IS 'How a pipeline run was initiated: by a user or by the system.';

-- Pipeline definitions table: a workspace's detection/redaction configs.
CREATE TABLE workspace_pipelines (
    -- Primary identifier
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References
    workspace_id    UUID             NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    account_id      UUID             NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,

    -- Composite key target for workspace-scoped foreign keys (join tables).
    CONSTRAINT workspace_pipelines_workspace_id_id_key UNIQUE (workspace_id, id),

    -- URL identity, unique within the workspace (among live pipelines; enforced
    -- by a partial index below so a slug frees up after soft deletion): lowercase
    -- alphanumeric with single internal dashes, 3-32 characters.
    slug            TEXT             NOT NULL,
    CONSTRAINT workspace_pipelines_slug_length CHECK (length(slug) BETWEEN 3 AND 32),
    CONSTRAINT workspace_pipelines_slug_format CHECK (slug ~ '^[a-z0-9]+(-[a-z0-9]+)*$'),

    -- Core attributes
    display_name    TEXT             NOT NULL,
    CONSTRAINT workspace_pipelines_display_name_length CHECK (length(trim(display_name)) BETWEEN 2 AND 128),
    description     TEXT             DEFAULT NULL,
    CONSTRAINT workspace_pipelines_description_length CHECK (description IS NULL OR length(description) <= 500),
    status          PIPELINE_STATUS  NOT NULL DEFAULT 'draft',

    -- Engine detection + redaction config (nvisy_schema plan as JSON):
    -- recognizers, enrichers, deduplication, label catalog, default scope.
    -- Policy references are relational (workspace_pipeline_policies, declared
    -- alongside policies), not embedded here.
    definition      JSONB            NOT NULL,
    CONSTRAINT workspace_pipelines_definition_size CHECK (length(definition::TEXT) BETWEEN 2 AND 1048576),

    -- Free-form metadata for filtering and display.
    metadata        JSONB            NOT NULL DEFAULT '{}',
    CONSTRAINT workspace_pipelines_metadata_size CHECK (length(metadata::TEXT) BETWEEN 2 AND 65536),

    -- Lifecycle timestamps
    created_at      TIMESTAMPTZ      NOT NULL DEFAULT current_timestamp,
    updated_at      TIMESTAMPTZ      NOT NULL DEFAULT current_timestamp,
    deleted_at      TIMESTAMPTZ      DEFAULT NULL,
    CONSTRAINT workspace_pipelines_updated_after_created CHECK (updated_at >= created_at),
    CONSTRAINT workspace_pipelines_deleted_after_created CHECK (deleted_at IS NULL OR deleted_at >= created_at)
);

-- Maintain updated_at on every row modification.
SELECT setup_updated_at('workspace_pipelines');

-- One live pipeline per slug within a workspace (slug frees up after deletion).
CREATE UNIQUE INDEX workspace_pipelines_slug_unique_idx
    ON workspace_pipelines (workspace_id, slug)
    WHERE deleted_at IS NULL;

-- Live pipelines of a workspace, newest first (the pipeline list).
CREATE INDEX workspace_pipelines_workspace_idx
    ON workspace_pipelines (workspace_id, created_at DESC)
    WHERE deleted_at IS NULL;

-- Live pipelines created by an account, newest first.
CREATE INDEX workspace_pipelines_account_idx
    ON workspace_pipelines (account_id, created_at DESC)
    WHERE deleted_at IS NULL;

-- Filter live pipelines by lifecycle status within a workspace.
CREATE INDEX workspace_pipelines_status_idx
    ON workspace_pipelines (status, workspace_id)
    WHERE deleted_at IS NULL;

-- Trigram search over live pipeline display names.
CREATE INDEX workspace_pipelines_display_name_trgm_idx
    ON workspace_pipelines USING gin (display_name gin_trgm_ops)
    WHERE deleted_at IS NULL;

COMMENT ON TABLE workspace_pipelines IS 'Workspace-scoped redaction pipeline definitions.';
COMMENT ON COLUMN workspace_pipelines.id IS 'Unique pipeline identifier';
COMMENT ON COLUMN workspace_pipelines.workspace_id IS 'Workspace this pipeline belongs to';
COMMENT ON COLUMN workspace_pipelines.account_id IS 'Account that created the pipeline';
COMMENT ON COLUMN workspace_pipelines.slug IS 'URL identity, unique among live pipelines in the workspace';
COMMENT ON COLUMN workspace_pipelines.display_name IS 'Pipeline display name (2-128 chars)';
COMMENT ON COLUMN workspace_pipelines.description IS 'Pipeline description (up to 500 chars)';
COMMENT ON COLUMN workspace_pipelines.status IS 'Pipeline lifecycle status';
COMMENT ON COLUMN workspace_pipelines.definition IS 'Detection/redaction config (nvisy_schema plan as JSON)';
COMMENT ON COLUMN workspace_pipelines.metadata IS 'Free-form metadata for filtering/display';
COMMENT ON COLUMN workspace_pipelines.created_at IS 'Pipeline creation timestamp';
COMMENT ON COLUMN workspace_pipelines.updated_at IS 'Last modification timestamp';
COMMENT ON COLUMN workspace_pipelines.deleted_at IS 'Soft-deletion timestamp; NULL means live';

-- Pipeline runs table: one pass of a file through a pipeline.
CREATE TABLE workspace_pipeline_runs (
    -- Primary identifier
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References
    pipeline_id     UUID                    NOT NULL REFERENCES workspace_pipelines (id) ON DELETE CASCADE,
    account_id      UUID                    NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,

    -- The three files a run relates to, each a distinct role. The input is the
    -- source document (required); the audit blob and redacted output are produced
    -- by the run, so they are null until their phase completes.
    --   input:  the original document being analyzed/redacted.
    --   audit:  the engine's analysis (Audit), a `file_kind = audit` file held
    --           between detect and redact; redact reads it as the source of truth.
    --   output: the redacted document produced by redact.
    -- A run is append-only audit history: its file references survive the files
    -- themselves. Files are only ever soft-deleted (their objects purged), so
    -- these ON DELETE actions fire only on a hard delete. The app never hard-
    -- deletes an individual file; the one hard delete is a whole-workspace
    -- teardown, which cascades files and runs away together. The input therefore
    -- cascades — a workspace deletion that removes the source document should
    -- remove its runs too. Produced files use ON DELETE SET NULL so that, in that
    -- same teardown, they clear rather than cascade (redundant here, but the
    -- correct action for a produced artifact).
    input_file_id   UUID                    NOT NULL REFERENCES workspace_files (id) ON DELETE CASCADE,
    audit_file_id   UUID                    DEFAULT NULL REFERENCES workspace_files (id) ON DELETE SET NULL,
    output_file_id  UUID                    DEFAULT NULL REFERENCES workspace_files (id) ON DELETE SET NULL,

    -- Run attributes
    trigger_type    PIPELINE_TRIGGER_TYPE   NOT NULL DEFAULT 'user',
    status          PIPELINE_RUN_STATUS     NOT NULL DEFAULT 'queued',

    -- Idempotency key from the initiating detect request; a repeat replays the
    -- existing run instead of analyzing twice.
    idempotency_key TEXT                    DEFAULT NULL,
    CONSTRAINT workspace_pipeline_runs_idempotency_key_length CHECK (idempotency_key IS NULL OR length(idempotency_key) BETWEEN 1 AND 255),

    -- Non-encrypted metadata for filtering and display. The engine's full
    -- per-recognizer usage report (durations, per-model token counts) is kept
    -- here under `usage` for drill-down; per-model token totals for usage
    -- aggregation live in the workspace_pipeline_run_usage table below.
    metadata        JSONB                   NOT NULL DEFAULT '{}',
    CONSTRAINT workspace_pipeline_runs_metadata_size CHECK (length(metadata::TEXT) BETWEEN 2 AND 65536),

    -- Detection lease: when a worker last claimed this run. A redelivered job
    -- whose claim is still fresh is skipped (no double-analyze); a stale claim
    -- (a worker that died mid-analysis) can be re-claimed. Null until claimed.
    claimed_at      TIMESTAMPTZ             DEFAULT NULL,

    -- Timing
    started_at      TIMESTAMPTZ             NOT NULL DEFAULT current_timestamp,
    completed_at    TIMESTAMPTZ             DEFAULT NULL,
    CONSTRAINT workspace_pipeline_runs_completed_after_started CHECK (completed_at IS NULL OR completed_at >= started_at)
);

-- A pipeline's runs, newest first (the run list).
CREATE INDEX workspace_pipeline_runs_pipeline_idx
    ON workspace_pipeline_runs (pipeline_id, started_at DESC);

-- Runs triggered by an account, newest first.
CREATE INDEX workspace_pipeline_runs_account_idx
    ON workspace_pipeline_runs (account_id, started_at DESC);

-- In-flight runs by status (queue and review backlog).
CREATE INDEX workspace_pipeline_runs_status_idx
    ON workspace_pipeline_runs (status, started_at DESC)
    WHERE status IN ('queued', 'analyzing', 'analyzed');

-- Runs analyzing a given input file, newest first.
CREATE INDEX workspace_pipeline_runs_input_file_idx
    ON workspace_pipeline_runs (input_file_id, started_at DESC);

-- Idempotent detect: at most one run per (pipeline, idempotency key).
CREATE UNIQUE INDEX workspace_pipeline_runs_idempotency_idx
    ON workspace_pipeline_runs (pipeline_id, idempotency_key)
    WHERE idempotency_key IS NOT NULL;

-- A run is append-only history: its file references (input/audit/output) are
-- kept even after those files are deleted, so the record of what a run analyzed
-- and produced survives. The `ON DELETE SET NULL` FKs would fire only on a hard
-- file delete, which never happens (files are soft-deleted); a reference to a
-- soft-deleted file resolves to "gone" at read time, distinct from a NULL that
-- means the run never had one.

COMMENT ON TABLE workspace_pipeline_runs IS 'Detect/redact runs: one pass of a file through a pipeline.';
COMMENT ON COLUMN workspace_pipeline_runs.id IS 'Unique run identifier';
COMMENT ON COLUMN workspace_pipeline_runs.pipeline_id IS 'Pipeline whose config drove the run';
COMMENT ON COLUMN workspace_pipeline_runs.account_id IS 'Account that triggered the run';
COMMENT ON COLUMN workspace_pipeline_runs.input_file_id IS 'Source document the run analyzes / redacts';
COMMENT ON COLUMN workspace_pipeline_runs.audit_file_id IS 'Audit file (file_kind=audit) holding the analysis between detect and redact';
COMMENT ON COLUMN workspace_pipeline_runs.output_file_id IS 'Redacted document produced by redact; NULL until completed';
COMMENT ON COLUMN workspace_pipeline_runs.trigger_type IS 'How the run was initiated';
COMMENT ON COLUMN workspace_pipeline_runs.status IS 'Current run status';
COMMENT ON COLUMN workspace_pipeline_runs.idempotency_key IS 'Detect idempotency key (dedupes retries)';
COMMENT ON COLUMN workspace_pipeline_runs.metadata IS 'Non-encrypted metadata for filtering/display; holds the full per-recognizer usage report under `usage`';
COMMENT ON COLUMN workspace_pipeline_runs.claimed_at IS 'Detection lease: when a worker last claimed this run';
COMMENT ON COLUMN workspace_pipeline_runs.started_at IS 'When the run started';
COMMENT ON COLUMN workspace_pipeline_runs.completed_at IS 'When the run completed; NULL while in flight';

-- Per-model inference usage for a run: one row per distinct model a run's
-- recognizers used. Token counts are aggregated across the recognizers that
-- shared a model, letting usage analytics report tokens broken down by model
-- (a run's summed tokens cannot, since a run may mix models). The full
-- per-recognizer report is kept on the run (metadata.usage) for drill-down;
-- this table is the aggregation surface.
CREATE TABLE workspace_pipeline_run_usage (
    -- Primary identifier
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- The run this usage belongs to; usage is deleted with its run.
    run_id          UUID                    NOT NULL REFERENCES workspace_pipeline_runs (id) ON DELETE CASCADE,

    -- The model and its optional version. Identity is (run, model, version):
    -- a run may use the same model at more than one version, and each is its own
    -- row (uniqueness enforced by an index below that normalizes a NULL version).
    model           TEXT                    NOT NULL,
    version         TEXT                    DEFAULT NULL,
    CONSTRAINT workspace_pipeline_run_usage_model_length CHECK (length(model) BETWEEN 1 AND 255),
    CONSTRAINT workspace_pipeline_run_usage_version_length CHECK (version IS NULL OR length(version) BETWEEN 1 AND 255),

    -- Token counts as the provider reported them. Each is independently nullable:
    -- `total` is NOT necessarily input + output (a provider may report only a
    -- total, or a total that also counts cached/reasoning tokens), so all three
    -- are carried faithfully and never derived from one another.
    input_tokens    BIGINT                  DEFAULT NULL,
    output_tokens   BIGINT                  DEFAULT NULL,
    total_tokens    BIGINT                  DEFAULT NULL,
    CONSTRAINT workspace_pipeline_run_usage_input_non_negative CHECK (input_tokens IS NULL OR input_tokens >= 0),
    CONSTRAINT workspace_pipeline_run_usage_output_non_negative CHECK (output_tokens IS NULL OR output_tokens >= 0),
    CONSTRAINT workspace_pipeline_run_usage_total_non_negative CHECK (total_tokens IS NULL OR total_tokens >= 0),

    -- Wall-clock time this model's recognizers spent, in milliseconds.
    duration_ms     BIGINT                  NOT NULL DEFAULT 0,
    CONSTRAINT workspace_pipeline_run_usage_duration_non_negative CHECK (duration_ms >= 0)
);

-- One row per (run, model, version); a NULL version is normalized so two
-- unversioned rows for the same model collide instead of both being inserted.
-- Leads with run_id, so it also serves per-run drill-down lookups.
CREATE UNIQUE INDEX workspace_pipeline_run_usage_run_model_version_key
    ON workspace_pipeline_run_usage (run_id, model, COALESCE(version, ''));

-- Usage rollups by model across runs.
CREATE INDEX workspace_pipeline_run_usage_model_idx
    ON workspace_pipeline_run_usage (model);

COMMENT ON TABLE workspace_pipeline_run_usage IS 'Per-model inference token usage for a pipeline run.';
COMMENT ON COLUMN workspace_pipeline_run_usage.id IS 'Unique usage row identifier';
COMMENT ON COLUMN workspace_pipeline_run_usage.run_id IS 'Run this usage belongs to';
COMMENT ON COLUMN workspace_pipeline_run_usage.model IS 'Model identifier the recognizers used';
COMMENT ON COLUMN workspace_pipeline_run_usage.version IS 'Model version, if the provider reported one';
COMMENT ON COLUMN workspace_pipeline_run_usage.input_tokens IS 'Input/prompt tokens for this model; NULL if not reported';
COMMENT ON COLUMN workspace_pipeline_run_usage.output_tokens IS 'Output/completion tokens for this model; NULL if not reported';
COMMENT ON COLUMN workspace_pipeline_run_usage.total_tokens IS 'Total tokens as reported (not necessarily input + output); NULL if not reported';
COMMENT ON COLUMN workspace_pipeline_run_usage.duration_ms IS 'Wall-clock time this model spent, in milliseconds';
