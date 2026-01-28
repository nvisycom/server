-- Pipeline: Workflow definitions, connections, and execution tracking
-- This migration creates tables for user-defined processing pipelines

-- Workspace connections table (encrypted provider credentials + context)
CREATE TABLE workspace_connections (
    -- Primary identifier
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References
    workspace_id    UUID            NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    account_id      UUID            NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,

    -- Core attributes
    name            TEXT            NOT NULL,
    provider        TEXT            NOT NULL,

    CONSTRAINT workspace_connections_name_length CHECK (length(trim(name)) BETWEEN 1 AND 255),
    CONSTRAINT workspace_connections_provider_length CHECK (length(trim(provider)) BETWEEN 1 AND 64),

    -- Encrypted connection data (XChaCha20-Poly1305 encrypted JSON)
    -- Contains: {"type": "postgres", "credentials": {...}, "context": {...}}
    -- The context includes resumption state (last cursor, offset, etc.)
    encrypted_data  BYTEA           NOT NULL,

    CONSTRAINT workspace_connections_data_size CHECK (length(encrypted_data) BETWEEN 1 AND 65536),

    -- Lifecycle timestamps
    created_at      TIMESTAMPTZ     NOT NULL DEFAULT current_timestamp,
    updated_at      TIMESTAMPTZ     NOT NULL DEFAULT current_timestamp,
    deleted_at      TIMESTAMPTZ     DEFAULT NULL,

    CONSTRAINT workspace_connections_updated_after_created CHECK (updated_at >= created_at),
    CONSTRAINT workspace_connections_deleted_after_created CHECK (deleted_at IS NULL OR deleted_at >= created_at)
);

-- Triggers
SELECT setup_updated_at('workspace_connections');

-- Indexes
CREATE INDEX workspace_connections_workspace_idx
    ON workspace_connections (workspace_id, created_at DESC)
    WHERE deleted_at IS NULL;

CREATE INDEX workspace_connections_provider_idx
    ON workspace_connections (provider, workspace_id)
    WHERE deleted_at IS NULL;

CREATE UNIQUE INDEX workspace_connections_name_unique_idx
    ON workspace_connections (workspace_id, lower(trim(name)))
    WHERE deleted_at IS NULL;

-- Comments
COMMENT ON TABLE workspace_connections IS
    'Encrypted provider connections (credentials + context) scoped to workspaces.';

COMMENT ON COLUMN workspace_connections.id IS 'Unique connection identifier';
COMMENT ON COLUMN workspace_connections.workspace_id IS 'Parent workspace reference';
COMMENT ON COLUMN workspace_connections.account_id IS 'Creator account reference';
COMMENT ON COLUMN workspace_connections.name IS 'Human-readable connection name (1-255 chars)';
COMMENT ON COLUMN workspace_connections.provider IS 'Provider type (openai, postgres, s3, pinecone, etc.)';
COMMENT ON COLUMN workspace_connections.encrypted_data IS 'XChaCha20-Poly1305 encrypted JSON with credentials and context';
COMMENT ON COLUMN workspace_connections.created_at IS 'Creation timestamp';
COMMENT ON COLUMN workspace_connections.updated_at IS 'Last modification timestamp';
COMMENT ON COLUMN workspace_connections.deleted_at IS 'Soft deletion timestamp';

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

-- Workspace pipeline definitions table
CREATE TABLE workspace_pipelines (
    -- Primary identifier
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References
    workspace_id    UUID             NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    account_id      UUID             NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,

    -- Core attributes
    name            TEXT             NOT NULL,
    description     TEXT             DEFAULT NULL,
    status          PIPELINE_STATUS  NOT NULL DEFAULT 'draft',

    CONSTRAINT workspace_pipelines_name_length CHECK (length(trim(name)) BETWEEN 1 AND 255),
    CONSTRAINT workspace_pipelines_description_length CHECK (description IS NULL OR length(description) <= 4096),

    -- Pipeline definition (flexible JSONB structure)
    -- Contains: steps[], input_schema, output_schema, variables, etc.
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
CREATE INDEX workspace_pipelines_workspace_idx
    ON workspace_pipelines (workspace_id, created_at DESC)
    WHERE deleted_at IS NULL;

CREATE INDEX workspace_pipelines_account_idx
    ON workspace_pipelines (account_id, created_at DESC)
    WHERE deleted_at IS NULL;

CREATE INDEX workspace_pipelines_status_idx
    ON workspace_pipelines (status, workspace_id)
    WHERE deleted_at IS NULL;

CREATE INDEX workspace_pipelines_name_trgm_idx
    ON workspace_pipelines USING gin (name gin_trgm_ops)
    WHERE deleted_at IS NULL;

-- Comments
COMMENT ON TABLE workspace_pipelines IS
    'User-defined processing pipeline definitions with step configurations.';

COMMENT ON COLUMN workspace_pipelines.id IS 'Unique pipeline identifier';
COMMENT ON COLUMN workspace_pipelines.workspace_id IS 'Parent workspace reference';
COMMENT ON COLUMN workspace_pipelines.account_id IS 'Creator account reference';
COMMENT ON COLUMN workspace_pipelines.name IS 'Pipeline name (1-255 chars)';
COMMENT ON COLUMN workspace_pipelines.description IS 'Pipeline description (up to 4096 chars)';
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
CREATE TABLE pipeline_runs (
    -- Primary identifier
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References
    pipeline_id     UUID                    NOT NULL REFERENCES workspace_pipelines (id) ON DELETE CASCADE,
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
    JOIN workspace_pipelines p ON pr.pipeline_id = p.id
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
    JOIN workspace_pipelines p ON pr.pipeline_id = p.id
WHERE pr.status IN ('completed', 'failed', 'cancelled')
ORDER BY pr.completed_at DESC;

COMMENT ON VIEW pipeline_run_history IS
    'Completed pipeline runs for history and analytics.';

-- Artifact type enum
CREATE TYPE ARTIFACT_TYPE AS ENUM (
    'input',        -- Input data for the run
    'output',       -- Final output data
    'intermediate'  -- Intermediate data between nodes
);

COMMENT ON TYPE ARTIFACT_TYPE IS
    'Classification of pipeline run artifacts.';

-- Pipeline artifacts table
CREATE TABLE pipeline_artifacts (
    -- Primary identifier
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References
    run_id          UUID            NOT NULL REFERENCES pipeline_runs (id) ON DELETE CASCADE,
    file_id         UUID            NOT NULL REFERENCES workspace_files (id) ON DELETE CASCADE,

    -- Artifact attributes
    artifact_type   ARTIFACT_TYPE   NOT NULL,

    -- Metadata
    metadata        JSONB           NOT NULL DEFAULT '{}',

    CONSTRAINT pipeline_artifacts_metadata_size CHECK (length(metadata::TEXT) BETWEEN 2 AND 65536),

    -- Timestamps
    created_at      TIMESTAMPTZ     NOT NULL DEFAULT current_timestamp
);

-- Indexes
CREATE INDEX pipeline_artifacts_run_idx
    ON pipeline_artifacts (run_id, artifact_type);

CREATE INDEX pipeline_artifacts_file_idx
    ON pipeline_artifacts (file_id);

-- Comments
COMMENT ON TABLE pipeline_artifacts IS
    'Artifacts produced during pipeline runs (inputs, outputs, intermediates).';

COMMENT ON COLUMN pipeline_artifacts.id IS 'Unique artifact identifier';
COMMENT ON COLUMN pipeline_artifacts.run_id IS 'Reference to pipeline run';
COMMENT ON COLUMN pipeline_artifacts.file_id IS 'Reference to file storing the artifact data';
COMMENT ON COLUMN pipeline_artifacts.artifact_type IS 'Type of artifact (input, output, intermediate)';
COMMENT ON COLUMN pipeline_artifacts.metadata IS 'Extended metadata (checksums, counts, etc.)';
COMMENT ON COLUMN pipeline_artifacts.created_at IS 'Creation timestamp';
