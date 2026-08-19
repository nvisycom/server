-- Policies: structured redaction policy definitions (nvisy_schema Policy) the
-- engine consults. A standalone workspace resource. The definition holds a
-- nvisy_schema::policy::Policy the engine consumes.

-- Workspace policies table: structured redaction governance config.
CREATE TABLE workspace_policies (
    -- Primary identifier
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References
    workspace_id    UUID            NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    account_id      UUID            NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,

    -- Composite key target for workspace-scoped foreign keys (join tables).
    CONSTRAINT workspace_policies_workspace_id_id_key UNIQUE (workspace_id, id),

    -- URL identity, unique within the workspace (among live policies; enforced by
    -- a partial index below so a slug frees up after soft deletion): lowercase
    -- alphanumeric with single internal dashes, 3-32 characters.
    slug            TEXT            NOT NULL,
    CONSTRAINT workspace_policies_slug_length CHECK (length(slug) BETWEEN 3 AND 32),
    CONSTRAINT workspace_policies_slug_format CHECK (slug ~ '^[a-z0-9]+(-[a-z0-9]+)*$'),

    -- Core attributes
    display_name    TEXT            NOT NULL,
    CONSTRAINT workspace_policies_display_name_length CHECK (length(trim(display_name)) BETWEEN 1 AND 255),
    description     TEXT            DEFAULT NULL,
    CONSTRAINT workspace_policies_description_length CHECK (description IS NULL OR length(description) <= 4096),

    -- Policy body (nvisy_schema::policy::PolicyDefinition as JSON: rules, labels,
    -- fallback, retention, `when` predicate). Stored XChaCha20-Poly1305 encrypted
    -- with the workspace-derived key.
    definition      BYTEA           NOT NULL,
    CONSTRAINT workspace_policies_definition_size CHECK (length(definition) BETWEEN 1 AND 1048576),

    -- Metadata (for filtering/display).
    metadata        JSONB           NOT NULL DEFAULT '{}',
    CONSTRAINT workspace_policies_metadata_size CHECK (length(metadata::TEXT) BETWEEN 2 AND 65536),

    -- Lifecycle timestamps
    created_at      TIMESTAMPTZ     NOT NULL DEFAULT current_timestamp,
    updated_at      TIMESTAMPTZ     NOT NULL DEFAULT current_timestamp,
    deleted_at      TIMESTAMPTZ     DEFAULT NULL,
    CONSTRAINT workspace_policies_updated_after_created CHECK (updated_at >= created_at),
    CONSTRAINT workspace_policies_deleted_after_created CHECK (deleted_at IS NULL OR deleted_at >= created_at)
);

-- Keep updated_at current on every row change.
SELECT setup_updated_at('workspace_policies');

-- Most recent live policies per workspace.
CREATE INDEX workspace_policies_workspace_idx
    ON workspace_policies (workspace_id, created_at DESC)
    WHERE deleted_at IS NULL;

-- Live policies created by an account, most recent first.
CREATE INDEX workspace_policies_account_idx
    ON workspace_policies (account_id, created_at DESC)
    WHERE deleted_at IS NULL;

-- One live slug per workspace (frees up after soft deletion).
CREATE UNIQUE INDEX workspace_policies_slug_unique_idx
    ON workspace_policies (workspace_id, slug)
    WHERE deleted_at IS NULL;

-- One live display name per workspace (case- and whitespace-insensitive).
CREATE UNIQUE INDEX workspace_policies_display_name_unique_idx
    ON workspace_policies (workspace_id, lower(trim(display_name)))
    WHERE deleted_at IS NULL;

COMMENT ON TABLE workspace_policies IS 'Structured redaction policies (nvisy_schema Policy) consumed by the engine.';
COMMENT ON COLUMN workspace_policies.id IS 'Unique policy identifier';
COMMENT ON COLUMN workspace_policies.workspace_id IS 'Parent workspace reference';
COMMENT ON COLUMN workspace_policies.account_id IS 'Creator account reference';
COMMENT ON COLUMN workspace_policies.slug IS 'URL identity, unique within the workspace (3-32 chars, dashed slug)';
COMMENT ON COLUMN workspace_policies.display_name IS 'Human-readable policy display name (1-255 chars)';
COMMENT ON COLUMN workspace_policies.description IS 'Policy description (up to 4096 chars)';
COMMENT ON COLUMN workspace_policies.definition IS 'Encrypted policy body (XChaCha20-Poly1305, workspace-derived key)';
COMMENT ON COLUMN workspace_policies.metadata IS 'Metadata for filtering/display';
COMMENT ON COLUMN workspace_policies.created_at IS 'Creation timestamp';
COMMENT ON COLUMN workspace_policies.updated_at IS 'Last modification timestamp';
COMMENT ON COLUMN workspace_policies.deleted_at IS 'Soft-deletion timestamp; NULL means live';

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
