-- Contexts: structured reference-data definitions (nvisy_schema Context)
-- the engine consults. A standalone workspace resource.

-- Workspace contexts table (structured reference-data for redaction)
-- The definition holds a nvisy_schema::context::Context the engine consumes.
CREATE TABLE workspace_contexts (
    -- Primary identifier
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References
    workspace_id    UUID            NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    account_id      UUID            NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,

    -- Composite key target for workspace-scoped foreign keys (join tables).
    CONSTRAINT workspace_contexts_workspace_id_id_key UNIQUE (workspace_id, id),

    -- Immutable URL identity, unique within the workspace. Mirrors the Slug
    -- newtype: lowercase alphanumeric with single internal dashes, 3-32 chars.
    slug            TEXT            NOT NULL,
    CONSTRAINT workspace_contexts_workspace_id_slug_key UNIQUE (workspace_id, slug),
    CONSTRAINT workspace_contexts_slug_length CHECK (length(slug) BETWEEN 3 AND 32),
    CONSTRAINT workspace_contexts_slug_format CHECK (slug ~ '^[a-z0-9]+(-[a-z0-9]+)*$'),

    -- Core attributes
    name            TEXT            NOT NULL,
    description     TEXT            DEFAULT NULL,
    version         TEXT            NOT NULL,

    CONSTRAINT workspace_contexts_name_length CHECK (length(trim(name)) BETWEEN 1 AND 255),
    CONSTRAINT workspace_contexts_description_length CHECK (description IS NULL OR length(description) <= 4096),
    CONSTRAINT workspace_contexts_version_length CHECK (length(trim(version)) BETWEEN 1 AND 64),

    -- Context body (nvisy_schema::context::Context as JSON: typed
    -- reference-data entries — biometric, geospatial, temporal, ...).
    -- Stored XChaCha20-Poly1305 encrypted with the workspace-derived key.
    definition      BYTEA           NOT NULL,

    CONSTRAINT workspace_contexts_definition_size CHECK (length(definition) BETWEEN 1 AND 1048576),

    -- Metadata (for filtering/display)
    metadata        JSONB           NOT NULL DEFAULT '{}',

    CONSTRAINT workspace_contexts_metadata_size CHECK (length(metadata::TEXT) BETWEEN 2 AND 65536),

    -- Lifecycle timestamps
    created_at      TIMESTAMPTZ     NOT NULL DEFAULT current_timestamp,
    updated_at      TIMESTAMPTZ     NOT NULL DEFAULT current_timestamp,
    deleted_at      TIMESTAMPTZ     DEFAULT NULL,

    CONSTRAINT workspace_contexts_updated_after_created CHECK (updated_at >= created_at),
    CONSTRAINT workspace_contexts_deleted_after_created CHECK (deleted_at IS NULL OR deleted_at >= created_at)
);

-- Triggers
SELECT setup_updated_at('workspace_contexts');

-- Indexes
CREATE INDEX workspace_contexts_workspace_idx
    ON workspace_contexts (workspace_id, created_at DESC)
    WHERE deleted_at IS NULL;

CREATE INDEX workspace_contexts_account_idx
    ON workspace_contexts (account_id, created_at DESC)
    WHERE deleted_at IS NULL;

CREATE UNIQUE INDEX workspace_contexts_name_unique_idx
    ON workspace_contexts (workspace_id, lower(trim(name)))
    WHERE deleted_at IS NULL;

-- Comments
COMMENT ON TABLE workspace_contexts IS
    'Structured reference-data contexts (nvisy_schema Context) consumed by the engine.';

COMMENT ON COLUMN workspace_contexts.id IS 'Unique context identifier';
COMMENT ON COLUMN workspace_contexts.workspace_id IS 'Parent workspace reference';
COMMENT ON COLUMN workspace_contexts.account_id IS 'Creator account reference';
COMMENT ON COLUMN workspace_contexts.name IS 'Human-readable context name (1-255 chars)';
COMMENT ON COLUMN workspace_contexts.description IS 'Context description (up to 4096 chars)';
COMMENT ON COLUMN workspace_contexts.version IS 'Semver of the context body';
COMMENT ON COLUMN workspace_contexts.definition IS 'Encrypted context body (XChaCha20-Poly1305, workspace-derived key)';
COMMENT ON COLUMN workspace_contexts.metadata IS 'Metadata for filtering/display';
COMMENT ON COLUMN workspace_contexts.created_at IS 'Creation timestamp';
COMMENT ON COLUMN workspace_contexts.updated_at IS 'Last modification timestamp';
COMMENT ON COLUMN workspace_contexts.deleted_at IS 'Soft deletion timestamp';
