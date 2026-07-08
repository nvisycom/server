-- Policies: structured redaction policy definitions (nvisy_schema Policy)
-- the engine consults. A standalone workspace resource.

-- Workspace redaction policies table (structured governance config)
-- The definition holds a nvisy_schema::policy::Policy the engine consumes.
CREATE TABLE workspace_policies (
    -- Primary identifier
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References
    workspace_id    UUID            NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    account_id      UUID            NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,

    -- Composite key target for workspace-scoped foreign keys (join tables).
    UNIQUE (workspace_id, id),

    -- Core attributes
    name            TEXT            NOT NULL,
    description     TEXT            DEFAULT NULL,
    version         TEXT            NOT NULL,

    CONSTRAINT workspace_policies_name_length CHECK (length(trim(name)) BETWEEN 1 AND 255),
    CONSTRAINT workspace_policies_description_length CHECK (description IS NULL OR length(description) <= 4096),
    CONSTRAINT workspace_policies_version_length CHECK (length(trim(version)) BETWEEN 1 AND 64),

    -- Policy body (nvisy_schema::policy::Policy as JSON: rules, labels,
    -- fallback, retention, applies_when predicate). Stored XChaCha20-Poly1305
    -- encrypted with the workspace-derived key.
    definition      BYTEA           NOT NULL,

    CONSTRAINT workspace_policies_definition_size CHECK (length(definition) BETWEEN 1 AND 1048576),

    -- Metadata (for filtering/display)
    metadata        JSONB           NOT NULL DEFAULT '{}',

    CONSTRAINT workspace_policies_metadata_size CHECK (length(metadata::TEXT) BETWEEN 2 AND 65536),

    -- Lifecycle timestamps
    created_at      TIMESTAMPTZ     NOT NULL DEFAULT current_timestamp,
    updated_at      TIMESTAMPTZ     NOT NULL DEFAULT current_timestamp,
    deleted_at      TIMESTAMPTZ     DEFAULT NULL,

    CONSTRAINT workspace_policies_updated_after_created CHECK (updated_at >= created_at),
    CONSTRAINT workspace_policies_deleted_after_created CHECK (deleted_at IS NULL OR deleted_at >= created_at)
);

-- Triggers
SELECT setup_updated_at('workspace_policies');

-- Indexes
CREATE INDEX workspace_policies_workspace_idx
    ON workspace_policies (workspace_id, created_at DESC)
    WHERE deleted_at IS NULL;

CREATE INDEX workspace_policies_account_idx
    ON workspace_policies (account_id, created_at DESC)
    WHERE deleted_at IS NULL;

CREATE UNIQUE INDEX workspace_policies_name_unique_idx
    ON workspace_policies (workspace_id, lower(trim(name)))
    WHERE deleted_at IS NULL;

-- Comments
COMMENT ON TABLE workspace_policies IS
    'Structured redaction policies (nvisy_schema Policy) consumed by the engine.';

COMMENT ON COLUMN workspace_policies.id IS 'Unique policy identifier';
COMMENT ON COLUMN workspace_policies.workspace_id IS 'Parent workspace reference';
COMMENT ON COLUMN workspace_policies.account_id IS 'Creator account reference';
COMMENT ON COLUMN workspace_policies.name IS 'Human-readable policy name (1-255 chars)';
COMMENT ON COLUMN workspace_policies.description IS 'Policy description (up to 4096 chars)';
COMMENT ON COLUMN workspace_policies.version IS 'Semver of the policy body';
COMMENT ON COLUMN workspace_policies.definition IS 'Encrypted policy body (XChaCha20-Poly1305, workspace-derived key)';
COMMENT ON COLUMN workspace_policies.metadata IS 'Metadata for filtering/display';
COMMENT ON COLUMN workspace_policies.created_at IS 'Creation timestamp';
COMMENT ON COLUMN workspace_policies.updated_at IS 'Last modification timestamp';
COMMENT ON COLUMN workspace_policies.deleted_at IS 'Soft deletion timestamp';

