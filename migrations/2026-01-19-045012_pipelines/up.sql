-- Pipeline: Workflow definitions and execution tracking
-- This migration creates tables for user-defined processing pipelines

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
    'queued',       -- Run is waiting to start
    'running',      -- Run is in progress
    'completed',    -- Run finished successfully
    'failed',       -- Run failed with error
    'cancelled'     -- Run was cancelled by user
);

COMMENT ON TYPE PIPELINE_RUN_STATUS IS
    'Execution status for pipeline runs.';

-- Pipeline run trigger type enum
CREATE TYPE PIPELINE_TRIGGER_TYPE AS ENUM (
    'manual',       -- Manually triggered by user
    'source',       -- Triggered by source connector (upload, webhook, etc.)
    'scheduled'     -- Triggered by schedule (future)
);

COMMENT ON TYPE PIPELINE_TRIGGER_TYPE IS
    'How a pipeline run was initiated.';

-- Pipeline definitions table
CREATE TABLE pipelines (
    -- Primary identifier
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References
    workspace_id    UUID             NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    account_id      UUID             NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,

    -- Core attributes
    name            TEXT             NOT NULL,
    description     TEXT             DEFAULT NULL,
    status          PIPELINE_STATUS  NOT NULL DEFAULT 'draft',

    CONSTRAINT pipelines_name_length CHECK (length(trim(name)) BETWEEN 1 AND 255),
    CONSTRAINT pipelines_description_length CHECK (description IS NULL OR length(description) <= 4096),

    -- Pipeline definition (flexible JSONB structure)
    -- Contains: steps[], input_schema, output_schema, variables, etc.
    definition      JSONB            NOT NULL DEFAULT '{"steps": []}',

    CONSTRAINT pipelines_definition_size CHECK (length(definition::TEXT) BETWEEN 2 AND 1048576),

    -- Configuration
    metadata        JSONB            NOT NULL DEFAULT '{}',

    CONSTRAINT pipelines_metadata_size CHECK (length(metadata::TEXT) BETWEEN 2 AND 65536),

    -- Lifecycle timestamps
    created_at      TIMESTAMPTZ      NOT NULL DEFAULT current_timestamp,
    updated_at      TIMESTAMPTZ      NOT NULL DEFAULT current_timestamp,
    deleted_at      TIMESTAMPTZ      DEFAULT NULL,

    CONSTRAINT pipelines_updated_after_created CHECK (updated_at >= created_at),
    CONSTRAINT pipelines_deleted_after_created CHECK (deleted_at IS NULL OR deleted_at >= created_at)
);

-- Triggers
SELECT setup_updated_at('pipelines');

-- Indexes
CREATE INDEX pipelines_workspace_idx
    ON pipelines (workspace_id, created_at DESC)
    WHERE deleted_at IS NULL;

CREATE INDEX pipelines_account_idx
    ON pipelines (account_id, created_at DESC)
    WHERE deleted_at IS NULL;

CREATE INDEX pipelines_status_idx
    ON pipelines (status, workspace_id)
    WHERE deleted_at IS NULL;

CREATE INDEX pipelines_name_trgm_idx
    ON pipelines USING gin (name gin_trgm_ops)
    WHERE deleted_at IS NULL;

-- Comments
COMMENT ON TABLE pipelines IS
    'User-defined processing pipeline definitions with step configurations.';

COMMENT ON COLUMN pipelines.id IS 'Unique pipeline identifier';
COMMENT ON COLUMN pipelines.workspace_id IS 'Parent workspace reference';
COMMENT ON COLUMN pipelines.account_id IS 'Creator account reference';
COMMENT ON COLUMN pipelines.name IS 'Pipeline name (1-255 chars)';
COMMENT ON COLUMN pipelines.description IS 'Pipeline description (up to 4096 chars)';
COMMENT ON COLUMN pipelines.status IS 'Pipeline lifecycle status';
COMMENT ON COLUMN pipelines.definition IS 'Pipeline definition JSON (steps, input/output schemas, etc.)';
COMMENT ON COLUMN pipelines.metadata IS 'Extended metadata';
COMMENT ON COLUMN pipelines.created_at IS 'Creation timestamp';
COMMENT ON COLUMN pipelines.updated_at IS 'Last modification timestamp';
COMMENT ON COLUMN pipelines.deleted_at IS 'Soft deletion timestamp';

-- Pipeline runs table (execution instances)
CREATE TABLE pipeline_runs (
    -- Primary identifier
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References
    pipeline_id     UUID                    NOT NULL REFERENCES pipelines (id) ON DELETE CASCADE,
    workspace_id    UUID                    NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    account_id      UUID                    NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,

    -- Run attributes
    trigger_type    PIPELINE_TRIGGER_TYPE   NOT NULL DEFAULT 'manual',
    status          PIPELINE_RUN_STATUS     NOT NULL DEFAULT 'queued',

    -- Input/output configuration for this run
    input_config    JSONB                   NOT NULL DEFAULT '{}',
    output_config   JSONB                   NOT NULL DEFAULT '{}',

    CONSTRAINT pipeline_runs_input_config_size CHECK (length(input_config::TEXT) BETWEEN 2 AND 262144),
    CONSTRAINT pipeline_runs_output_config_size CHECK (length(output_config::TEXT) BETWEEN 2 AND 262144),

    -- Snapshot of pipeline definition at run time (for reproducibility)
    definition_snapshot JSONB               NOT NULL DEFAULT '{}',

    CONSTRAINT pipeline_runs_definition_snapshot_size CHECK (length(definition_snapshot::TEXT) BETWEEN 2 AND 1048576),

    -- Error details (if failed)
    error           JSONB                   DEFAULT NULL,

    CONSTRAINT pipeline_runs_error_size CHECK (error IS NULL OR length(error::TEXT) <= 65536),

    -- Metrics
    metrics         JSONB                   NOT NULL DEFAULT '{}',

    CONSTRAINT pipeline_runs_metrics_size CHECK (length(metrics::TEXT) BETWEEN 2 AND 65536),

    -- Timing
    started_at      TIMESTAMPTZ             DEFAULT NULL,
    completed_at    TIMESTAMPTZ             DEFAULT NULL,
    created_at      TIMESTAMPTZ             NOT NULL DEFAULT current_timestamp,

    CONSTRAINT pipeline_runs_started_after_created CHECK (started_at IS NULL OR started_at >= created_at),
    CONSTRAINT pipeline_runs_completed_after_started CHECK (completed_at IS NULL OR (started_at IS NOT NULL AND completed_at >= started_at))
);

-- Indexes
CREATE INDEX pipeline_runs_pipeline_idx
    ON pipeline_runs (pipeline_id, created_at DESC);

CREATE INDEX pipeline_runs_workspace_idx
    ON pipeline_runs (workspace_id, created_at DESC);

CREATE INDEX pipeline_runs_account_idx
    ON pipeline_runs (account_id, created_at DESC);

CREATE INDEX pipeline_runs_status_idx
    ON pipeline_runs (status, created_at DESC)
    WHERE status IN ('queued', 'running');

CREATE INDEX pipeline_runs_trigger_idx
    ON pipeline_runs (trigger_type, workspace_id);

-- Comments
COMMENT ON TABLE pipeline_runs IS
    'Pipeline execution instances with status tracking and metrics.';

COMMENT ON COLUMN pipeline_runs.id IS 'Unique run identifier';
COMMENT ON COLUMN pipeline_runs.pipeline_id IS 'Reference to pipeline definition';
COMMENT ON COLUMN pipeline_runs.workspace_id IS 'Parent workspace reference';
COMMENT ON COLUMN pipeline_runs.account_id IS 'Account that triggered the run';
COMMENT ON COLUMN pipeline_runs.trigger_type IS 'How the run was initiated';
COMMENT ON COLUMN pipeline_runs.status IS 'Current execution status';
COMMENT ON COLUMN pipeline_runs.input_config IS 'Runtime input configuration';
COMMENT ON COLUMN pipeline_runs.output_config IS 'Runtime output configuration';
COMMENT ON COLUMN pipeline_runs.definition_snapshot IS 'Pipeline definition snapshot at run time';
COMMENT ON COLUMN pipeline_runs.error IS 'Error details if run failed';
COMMENT ON COLUMN pipeline_runs.metrics IS 'Run metrics (duration, resources, etc.)';
COMMENT ON COLUMN pipeline_runs.started_at IS 'When execution started';
COMMENT ON COLUMN pipeline_runs.completed_at IS 'When execution completed';
COMMENT ON COLUMN pipeline_runs.created_at IS 'When run was created/queued';

-- View for active pipeline runs
CREATE VIEW active_pipeline_runs AS
SELECT
    pr.id,
    pr.pipeline_id,
    p.name AS pipeline_name,
    pr.workspace_id,
    pr.account_id,
    pr.trigger_type,
    pr.status,
    pr.started_at,
    pr.created_at,
    EXTRACT(EPOCH FROM (COALESCE(pr.completed_at, current_timestamp) - pr.started_at)) AS duration_seconds
FROM pipeline_runs pr
    JOIN pipelines p ON pr.pipeline_id = p.id
WHERE pr.status IN ('queued', 'running')
ORDER BY pr.created_at DESC;

COMMENT ON VIEW active_pipeline_runs IS
    'Currently active pipeline runs with progress information.';

-- View for pipeline run history
CREATE VIEW pipeline_run_history AS
SELECT
    pr.id,
    pr.pipeline_id,
    p.name AS pipeline_name,
    pr.workspace_id,
    pr.trigger_type,
    pr.status,
    pr.started_at,
    pr.completed_at,
    EXTRACT(EPOCH FROM (pr.completed_at - pr.started_at)) AS duration_seconds,
    pr.error IS NOT NULL AS has_error,
    pr.created_at
FROM pipeline_runs pr
    JOIN pipelines p ON pr.pipeline_id = p.id
WHERE pr.status IN ('completed', 'failed', 'cancelled')
ORDER BY pr.completed_at DESC;

COMMENT ON VIEW pipeline_run_history IS
    'Completed pipeline runs for history and analytics.';
