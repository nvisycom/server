-- Pipelines: redaction pipeline definitions, their runs, and artifacts.
-- Policy references live in a join table declared alongside policies.

-- Pipeline status enum
CREATE TYPE PIPELINE_STATUS AS ENUM (
    'draft',        -- Pipeline is being configured
    'enabled',      -- Pipeline is ready to run
    'disabled'      -- Pipeline is disabled
);

COMMENT ON TYPE PIPELINE_STATUS IS
    'Lifecycle status for pipeline definitions.';

-- Pipeline run status enum
CREATE TYPE PIPELINE_RUN_STATUS AS ENUM (
    'queued',       -- Enqueued for detection; no worker has picked it up yet
    'analyzing',    -- A worker is actively analyzing the document
    'analyzed',     -- Detection done; awaiting reviewer verification
    'completed',    -- Redaction applied; run finished
    'failed',       -- Run failed with error
    'cancelled'     -- Run was cancelled by user
);

COMMENT ON TYPE PIPELINE_RUN_STATUS IS
    'Execution status for pipeline runs.';

-- Pipeline trigger type enum
CREATE TYPE PIPELINE_TRIGGER_TYPE AS ENUM (
    'user',         -- Started directly by a user
    'system'        -- Started automatically (e.g. a file upload auto-redacted)
);

COMMENT ON TYPE PIPELINE_TRIGGER_TYPE IS
    'How a pipeline run was initiated.';

-- Workspace pipeline definitions table
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
    description     TEXT             DEFAULT NULL,
    status          PIPELINE_STATUS  NOT NULL DEFAULT 'draft',

    CONSTRAINT workspace_pipelines_display_name_length CHECK (length(trim(display_name)) BETWEEN 2 AND 128),
    CONSTRAINT workspace_pipelines_description_length CHECK (description IS NULL OR length(description) <= 500),

    -- Engine detection + redaction config (nvisy_schema plan as JSON):
    -- recognizers, enrichers, deduplication, label catalog, default scope.
    -- Policy references are relational (workspace_pipeline_policies, declared
    -- alongside policies), not embedded here.
    definition      JSONB            NOT NULL,

    CONSTRAINT workspace_pipelines_definition_size CHECK (length(definition::TEXT) BETWEEN 2 AND 1048576),

    -- Configuration
    metadata        JSONB            NOT NULL DEFAULT '{}',

    CONSTRAINT workspace_pipelines_metadata_size CHECK (length(metadata::TEXT) BETWEEN 2 AND 65536),

    -- Scheduling (optional)
    schedule_cron   TEXT             DEFAULT NULL,
    schedule_tz     TEXT             DEFAULT 'UTC',
    next_run_at     TIMESTAMPTZ      DEFAULT NULL,

    CONSTRAINT workspace_pipelines_schedule_cron_length CHECK (schedule_cron IS NULL OR length(schedule_cron) BETWEEN 9 AND 100),
    CONSTRAINT workspace_pipelines_schedule_tz_length CHECK (length(schedule_tz) BETWEEN 1 AND 64),
    CONSTRAINT workspace_pipelines_schedule_requires_cron CHECK (next_run_at IS NULL OR schedule_cron IS NOT NULL),

    -- Lifecycle timestamps
    created_at      TIMESTAMPTZ      NOT NULL DEFAULT current_timestamp,
    updated_at      TIMESTAMPTZ      NOT NULL DEFAULT current_timestamp,
    deleted_at      TIMESTAMPTZ      DEFAULT NULL,

    CONSTRAINT workspace_pipelines_updated_after_created CHECK (updated_at >= created_at),
    CONSTRAINT workspace_pipelines_deleted_after_created CHECK (deleted_at IS NULL OR deleted_at >= created_at)
);

-- Triggers
SELECT setup_updated_at('workspace_pipelines');

-- Indexes
CREATE UNIQUE INDEX workspace_pipelines_slug_unique_idx
    ON workspace_pipelines (workspace_id, slug)
    WHERE deleted_at IS NULL;

CREATE INDEX workspace_pipelines_workspace_idx
    ON workspace_pipelines (workspace_id, created_at DESC)
    WHERE deleted_at IS NULL;

CREATE INDEX workspace_pipelines_account_idx
    ON workspace_pipelines (account_id, created_at DESC)
    WHERE deleted_at IS NULL;

CREATE INDEX workspace_pipelines_status_idx
    ON workspace_pipelines (status, workspace_id)
    WHERE deleted_at IS NULL;

CREATE INDEX workspace_pipelines_display_name_trgm_idx
    ON workspace_pipelines USING gin (display_name gin_trgm_ops)
    WHERE deleted_at IS NULL;

-- Comments
COMMENT ON TABLE workspace_pipelines IS
    'Redaction pipeline definitions with step configurations.';

COMMENT ON COLUMN workspace_pipelines.id IS 'Unique pipeline identifier';
COMMENT ON COLUMN workspace_pipelines.workspace_id IS 'Parent workspace reference';
COMMENT ON COLUMN workspace_pipelines.account_id IS 'Creator account reference';
COMMENT ON COLUMN workspace_pipelines.display_name IS 'Pipeline display name (2-128 chars)';
COMMENT ON COLUMN workspace_pipelines.description IS 'Pipeline description (up to 500 chars)';
COMMENT ON COLUMN workspace_pipelines.status IS 'Pipeline lifecycle status';
COMMENT ON COLUMN workspace_pipelines.definition IS 'Pipeline definition JSON (steps, input/output schemas, etc.)';
COMMENT ON COLUMN workspace_pipelines.metadata IS 'Extended metadata';
COMMENT ON COLUMN workspace_pipelines.schedule_cron IS 'Cron expression for scheduled runs (e.g., "0 0 * * *")';
COMMENT ON COLUMN workspace_pipelines.schedule_tz IS 'Timezone for schedule interpretation (default: UTC)';
COMMENT ON COLUMN workspace_pipelines.next_run_at IS 'Next scheduled run time (computed from cron)';
COMMENT ON COLUMN workspace_pipelines.created_at IS 'Creation timestamp';
COMMENT ON COLUMN workspace_pipelines.updated_at IS 'Last modification timestamp';
COMMENT ON COLUMN workspace_pipelines.deleted_at IS 'Soft deletion timestamp';

-- Pipeline runs table (execution instances)
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
    -- Produced files use ON DELETE SET NULL so deleting them (e.g. via retention)
    -- leaves the run history intact; the input cascades since a run is meaningless
    -- without its source document.
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

    -- Metadata (non-encrypted, for filtering/display)
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

-- Indexes
CREATE INDEX workspace_pipeline_runs_pipeline_idx
    ON workspace_pipeline_runs (pipeline_id, started_at DESC);

CREATE INDEX workspace_pipeline_runs_account_idx
    ON workspace_pipeline_runs (account_id, started_at DESC);

CREATE INDEX workspace_pipeline_runs_status_idx
    ON workspace_pipeline_runs (status, started_at DESC)
    WHERE status IN ('queued', 'analyzing', 'analyzed');

CREATE INDEX workspace_pipeline_runs_input_file_idx
    ON workspace_pipeline_runs (input_file_id, started_at DESC);

-- Idempotent detect: at most one run per (pipeline, idempotency key).
CREATE UNIQUE INDEX workspace_pipeline_runs_idempotency_idx
    ON workspace_pipeline_runs (pipeline_id, idempotency_key)
    WHERE idempotency_key IS NOT NULL;

-- Comments
COMMENT ON TABLE workspace_pipeline_runs IS
    'Detect/redact runs: one analysis of a file through a pipeline, awaiting review then redaction.';

COMMENT ON COLUMN workspace_pipeline_runs.id IS 'Unique run identifier';
COMMENT ON COLUMN workspace_pipeline_runs.pipeline_id IS 'Pipeline whose config drove the run';
COMMENT ON COLUMN workspace_pipeline_runs.account_id IS 'Account that triggered the run (optional)';
COMMENT ON COLUMN workspace_pipeline_runs.input_file_id IS 'Source document the run analyzes / redacts';
COMMENT ON COLUMN workspace_pipeline_runs.audit_file_id IS 'Audit file (file_kind=audit) holding the encrypted analysis between detect and redact';
COMMENT ON COLUMN workspace_pipeline_runs.output_file_id IS 'Redacted document produced by redact (file_kind=redacted); null until completed';
COMMENT ON COLUMN workspace_pipeline_runs.trigger_type IS 'How the run was initiated';
COMMENT ON COLUMN workspace_pipeline_runs.status IS 'Current run status';
COMMENT ON COLUMN workspace_pipeline_runs.idempotency_key IS 'Detect idempotency key (dedupes retries)';
COMMENT ON COLUMN workspace_pipeline_runs.metadata IS 'Non-encrypted metadata for filtering/display';
COMMENT ON COLUMN workspace_pipeline_runs.claimed_at IS 'Detection lease: when a worker last claimed this run';
COMMENT ON COLUMN workspace_pipeline_runs.started_at IS 'When the run started';
COMMENT ON COLUMN workspace_pipeline_runs.completed_at IS 'When the run completed';

-- View for active pipeline runs
CREATE VIEW active_workspace_pipeline_runs AS
SELECT
    pr.id,
    pr.pipeline_id,
    p.display_name AS pipeline_name,
    p.workspace_id,
    pr.account_id,
    pr.trigger_type,
    pr.status,
    pr.started_at,
    EXTRACT(EPOCH FROM (COALESCE(pr.completed_at, current_timestamp) - pr.started_at)) AS duration_seconds
FROM workspace_pipeline_runs pr
    JOIN workspace_pipelines p ON pr.pipeline_id = p.id
WHERE pr.status IN ('queued', 'analyzing', 'analyzed')
ORDER BY pr.started_at DESC NULLS LAST;

COMMENT ON VIEW active_workspace_pipeline_runs IS
    'Currently active pipeline runs with progress information.';

-- View for workspace pipeline run history
CREATE VIEW workspace_pipeline_run_history AS
SELECT
    pr.id,
    pr.pipeline_id,
    p.display_name AS pipeline_name,
    p.workspace_id,
    pr.trigger_type,
    pr.status,
    pr.started_at,
    pr.completed_at,
    EXTRACT(EPOCH FROM (pr.completed_at - pr.started_at)) AS duration_seconds
FROM workspace_pipeline_runs pr
    JOIN workspace_pipelines p ON pr.pipeline_id = p.id
WHERE pr.status IN ('completed', 'failed', 'cancelled')
ORDER BY pr.completed_at DESC;

COMMENT ON VIEW workspace_pipeline_run_history IS
    'Completed pipeline runs for history and analytics.';
