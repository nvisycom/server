-- Pipelines: redaction pipeline definitions. A pipeline is a workspace-scoped
-- detection/redaction config that references workspace policies (via the join
-- table below) and drives the detections created from it.

-- Lifecycle status of a pipeline definition.
CREATE TYPE PIPELINE_STATUS AS ENUM (
    'draft',        -- Pipeline is being configured
    'enabled',      -- Pipeline is ready to run
    'disabled'      -- Pipeline is turned off
);

COMMENT ON TYPE PIPELINE_STATUS IS 'Lifecycle status of a pipeline definition: draft, enabled, or disabled.';

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


-- Pipeline -> policy join table: redaction policies a pipeline applies.
-- Lives here (after both workspace_pipelines and workspace_policies exist); the
-- shared workspace_id in both composite foreign keys enforces that a pipeline
-- can only reference policies from its own workspace. Drops before its parent in
-- down.sql.
CREATE TABLE workspace_pipeline_policies (
    -- References
    workspace_id    UUID            NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    pipeline_id     UUID            NOT NULL,
    policy_id       UUID            NOT NULL,

    PRIMARY KEY (pipeline_id, policy_id),

    -- Composite foreign keys, both sharing workspace_id, so a pipeline can only
    -- reference policies from its own workspace.
    CONSTRAINT workspace_pipeline_policies_pipeline_fkey FOREIGN KEY (workspace_id, pipeline_id)
        REFERENCES workspace_pipelines (workspace_id, id) ON DELETE CASCADE,
    CONSTRAINT workspace_pipeline_policies_policy_fkey FOREIGN KEY (workspace_id, policy_id)
        REFERENCES workspace_policies (workspace_id, id) ON DELETE CASCADE
);

-- All pipelines that apply a given policy (back the policy composite FK).
CREATE INDEX workspace_pipeline_policies_policy_idx ON workspace_pipeline_policies (policy_id);

COMMENT ON TABLE workspace_pipeline_policies IS 'Policies a pipeline applies at redaction. CASCADE cleans up on hard delete.';
COMMENT ON COLUMN workspace_pipeline_policies.workspace_id IS 'Workspace shared by the pipeline and policy';
COMMENT ON COLUMN workspace_pipeline_policies.pipeline_id IS 'Pipeline that applies the policy';
COMMENT ON COLUMN workspace_pipeline_policies.policy_id IS 'Policy applied by the pipeline';
