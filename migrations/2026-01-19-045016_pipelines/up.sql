-- Pipelines: redaction pipeline definitions, their policy/context
-- references (join tables), runs, and artifacts.

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

-- Pipeline trigger type enum
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

    -- Composite key target for workspace-scoped foreign keys (join tables).
    UNIQUE (workspace_id, id),

    -- Core attributes
    name            TEXT             NOT NULL,
    description     TEXT             DEFAULT NULL,
    status          PIPELINE_STATUS  NOT NULL DEFAULT 'draft',

    CONSTRAINT workspace_pipelines_name_length CHECK (length(trim(name)) BETWEEN 1 AND 255),
    CONSTRAINT workspace_pipelines_description_length CHECK (description IS NULL OR length(description) <= 4096),

    -- Engine detection + redaction config (nvisy_schema plan as JSON):
    -- recognizers, enrichers, deduplication, label catalog, default scope.
    -- Policy/context references are relational (see the join tables below),
    -- not embedded here.
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
    'Redaction pipeline definitions with step configurations.';

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

-- Pipeline → policy references (redaction rules applied by the pipeline).
-- The shared workspace_id in both composite foreign keys enforces that a
-- pipeline can only reference policies from its own workspace.
CREATE TABLE pipeline_policies (
    workspace_id    UUID            NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    pipeline_id     UUID            NOT NULL,
    policy_id       UUID            NOT NULL,

    PRIMARY KEY (pipeline_id, policy_id),

    CONSTRAINT pipeline_policies_pipeline_fkey FOREIGN KEY (workspace_id, pipeline_id)
        REFERENCES workspace_pipelines (workspace_id, id) ON DELETE CASCADE,
    CONSTRAINT pipeline_policies_policy_fkey FOREIGN KEY (workspace_id, policy_id)
        REFERENCES workspace_policies (workspace_id, id) ON DELETE CASCADE
);

CREATE INDEX pipeline_policies_policy_idx ON pipeline_policies (policy_id);

COMMENT ON TABLE pipeline_policies IS
    'Policies a pipeline applies at redaction. CASCADE cleans up on hard delete.';

-- Pipeline → context references (reference data supplied to detection).
CREATE TABLE pipeline_contexts (
    workspace_id    UUID            NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    pipeline_id     UUID            NOT NULL,
    context_id      UUID            NOT NULL,

    PRIMARY KEY (pipeline_id, context_id),

    CONSTRAINT pipeline_contexts_pipeline_fkey FOREIGN KEY (workspace_id, pipeline_id)
        REFERENCES workspace_pipelines (workspace_id, id) ON DELETE CASCADE,
    CONSTRAINT pipeline_contexts_context_fkey FOREIGN KEY (workspace_id, context_id)
        REFERENCES workspace_contexts (workspace_id, id) ON DELETE CASCADE
);

CREATE INDEX pipeline_contexts_context_idx ON pipeline_contexts (context_id);

COMMENT ON TABLE pipeline_contexts IS
    'Contexts a pipeline supplies to detection. CASCADE cleans up on hard delete.';

-- Pipeline runs table (execution instances)
CREATE TABLE workspace_pipeline_runs (
    -- Primary identifier
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References
    pipeline_id     UUID                    NOT NULL REFERENCES workspace_pipelines (id) ON DELETE CASCADE,
    account_id      UUID                    REFERENCES accounts (id) ON DELETE SET NULL,

    -- Run attributes
    trigger_type    PIPELINE_TRIGGER_TYPE   NOT NULL DEFAULT 'manual',
    status          PIPELINE_RUN_STATUS     NOT NULL DEFAULT 'queued',

    -- Snapshot of pipeline definition at run time (for reproducibility)
    definition_snapshot JSONB               NOT NULL DEFAULT '{}',

    CONSTRAINT workspace_pipeline_runs_definition_snapshot_size CHECK (length(definition_snapshot::TEXT) BETWEEN 2 AND 1048576),

    -- Metadata (non-encrypted, for filtering/display)
    metadata        JSONB                   NOT NULL DEFAULT '{}',

    CONSTRAINT workspace_pipeline_runs_metadata_size CHECK (length(metadata::TEXT) BETWEEN 2 AND 65536),

    -- Execution logs
    logs            JSONB                   NOT NULL DEFAULT '[]',

    CONSTRAINT workspace_pipeline_runs_logs_size CHECK (length(logs::TEXT) BETWEEN 2 AND 1048576),

    -- Timing
    started_at      TIMESTAMPTZ             NOT NULL DEFAULT current_timestamp,
    completed_at    TIMESTAMPTZ             DEFAULT NULL,

    CONSTRAINT workspace_pipeline_runs_completed_after_started CHECK (completed_at IS NULL OR completed_at >= started_at)
);

-- Indexes
CREATE INDEX workspace_pipeline_runs_pipeline_idx
    ON workspace_pipeline_runs (pipeline_id, started_at DESC);

CREATE INDEX workspace_pipeline_runs_account_idx
    ON workspace_pipeline_runs (account_id, started_at DESC)
    WHERE account_id IS NOT NULL;

CREATE INDEX workspace_pipeline_runs_status_idx
    ON workspace_pipeline_runs (status, started_at DESC)
    WHERE status IN ('queued', 'running');

-- Comments
COMMENT ON TABLE workspace_pipeline_runs IS
    'Pipeline execution instances with status tracking and logs.';

COMMENT ON COLUMN workspace_pipeline_runs.id IS 'Unique run identifier';
COMMENT ON COLUMN workspace_pipeline_runs.pipeline_id IS 'Reference to pipeline definition';
COMMENT ON COLUMN workspace_pipeline_runs.account_id IS 'Account that triggered the run (optional)';
COMMENT ON COLUMN workspace_pipeline_runs.trigger_type IS 'How the run was initiated';
COMMENT ON COLUMN workspace_pipeline_runs.status IS 'Current execution status';
COMMENT ON COLUMN workspace_pipeline_runs.definition_snapshot IS 'Pipeline definition snapshot at run time';
COMMENT ON COLUMN workspace_pipeline_runs.metadata IS 'Non-encrypted metadata for filtering/display';
COMMENT ON COLUMN workspace_pipeline_runs.logs IS 'Execution logs as JSON array';
COMMENT ON COLUMN workspace_pipeline_runs.started_at IS 'When execution started';
COMMENT ON COLUMN workspace_pipeline_runs.completed_at IS 'When execution completed';

-- View for active pipeline runs
CREATE VIEW active_workspace_pipeline_runs AS
SELECT
    pr.id,
    pr.pipeline_id,
    p.name AS pipeline_name,
    p.workspace_id,
    pr.account_id,
    pr.trigger_type,
    pr.status,
    pr.started_at,
    EXTRACT(EPOCH FROM (COALESCE(pr.completed_at, current_timestamp) - pr.started_at)) AS duration_seconds
FROM workspace_pipeline_runs pr
    JOIN workspace_pipelines p ON pr.pipeline_id = p.id
WHERE pr.status IN ('queued', 'running')
ORDER BY pr.started_at DESC NULLS LAST;

COMMENT ON VIEW active_workspace_pipeline_runs IS
    'Currently active pipeline runs with progress information.';

-- View for workspace pipeline run history
CREATE VIEW workspace_pipeline_run_history AS
SELECT
    pr.id,
    pr.pipeline_id,
    p.name AS pipeline_name,
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

-- Artifact type enum
CREATE TYPE ARTIFACT_TYPE AS ENUM (
    'input',        -- Input data for the run
    'output',       -- Final output data
    'intermediate'  -- Intermediate data between nodes
);

COMMENT ON TYPE ARTIFACT_TYPE IS
    'Classification of pipeline run artifacts.';

-- Pipeline artifacts table
CREATE TABLE workspace_pipeline_artifacts (
    -- Primary identifier
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References
    run_id          UUID            NOT NULL REFERENCES workspace_pipeline_runs (id) ON DELETE CASCADE,
    file_id         UUID            NOT NULL REFERENCES workspace_files (id) ON DELETE CASCADE,

    -- Artifact attributes
    artifact_type   ARTIFACT_TYPE   NOT NULL,

    -- Metadata
    metadata        JSONB           NOT NULL DEFAULT '{}',

    CONSTRAINT workspace_pipeline_artifacts_metadata_size CHECK (length(metadata::TEXT) BETWEEN 2 AND 65536),

    -- Timestamps
    created_at      TIMESTAMPTZ     NOT NULL DEFAULT current_timestamp
);

-- Indexes
CREATE INDEX workspace_pipeline_artifacts_run_idx
    ON workspace_pipeline_artifacts (run_id, artifact_type);

CREATE INDEX workspace_pipeline_artifacts_file_idx
    ON workspace_pipeline_artifacts (file_id);

-- Comments
COMMENT ON TABLE workspace_pipeline_artifacts IS
    'Artifacts produced during pipeline runs (inputs, outputs, intermediates).';

COMMENT ON COLUMN workspace_pipeline_artifacts.id IS 'Unique artifact identifier';
COMMENT ON COLUMN workspace_pipeline_artifacts.run_id IS 'Reference to pipeline run';
COMMENT ON COLUMN workspace_pipeline_artifacts.file_id IS 'Reference to file storing the artifact data';
COMMENT ON COLUMN workspace_pipeline_artifacts.artifact_type IS 'Type of artifact (input, output, intermediate)';
COMMENT ON COLUMN workspace_pipeline_artifacts.metadata IS 'Extended metadata (checksums, counts, etc.)';
COMMENT ON COLUMN workspace_pipeline_artifacts.created_at IS 'Creation timestamp';
