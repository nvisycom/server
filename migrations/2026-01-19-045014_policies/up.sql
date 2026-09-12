-- Policies: structured redaction policy definitions (nvisy_schema Policy) the
-- engine consults. A standalone workspace resource. A policy is a stable logical
-- identity (slug, display name) whose content is versioned: the encrypted
-- definition lives in workspace_policy_versions, and current_version_id names the
-- live version. Editing the definition mints a new immutable version; a detection
-- pins the exact version it ran (see the detections migration).

-- Workspace policies table: the logical policy (stable identity), pointing at its
-- current version.
CREATE TABLE workspace_policies (
    -- Primary identifier
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References
    workspace_id    UUID            NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    account_id      UUID            NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,

    -- Composite key target for workspace-scoped foreign keys (join tables and the
    -- version table).
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

    -- The live version whose definition the engine consumes. NULL only transiently
    -- while a policy and its first version are created in one transaction; the
    -- foreign key to workspace_policy_versions is added by ALTER below, once that
    -- table exists.
    current_version_id UUID         DEFAULT NULL,

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
COMMENT ON COLUMN workspace_policies.current_version_id IS 'The live version whose definition the engine consumes';
COMMENT ON COLUMN workspace_policies.metadata IS 'Metadata for filtering/display';
COMMENT ON COLUMN workspace_policies.created_at IS 'Creation timestamp';
COMMENT ON COLUMN workspace_policies.updated_at IS 'Last modification timestamp';
COMMENT ON COLUMN workspace_policies.deleted_at IS 'Soft-deletion timestamp; NULL means live';

-- Policy versions: immutable snapshots of a policy's definition. Editing a
-- policy's definition inserts a new version and repoints current_version_id; a
-- version is never updated or deleted, so a detection that pinned it can always
-- reproduce the exact definition it ran (see the detections migration).
CREATE TABLE workspace_policy_versions (
    -- Primary identifier
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- The logical policy this is a version of. The composite foreign key pins the
    -- version to the policy's workspace.
    policy_id       UUID            NOT NULL,
    workspace_id    UUID            NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    CONSTRAINT workspace_policy_versions_policy_fkey
        FOREIGN KEY (workspace_id, policy_id)
        REFERENCES workspace_policies (workspace_id, id) ON DELETE CASCADE,

    -- Author of this version.
    account_id      UUID            NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,

    -- Monotonic per-policy version number, 1..N.
    version_number  INTEGER         NOT NULL,
    CONSTRAINT workspace_policy_versions_number_positive CHECK (version_number >= 1),
    CONSTRAINT workspace_policy_versions_policy_version_key UNIQUE (policy_id, version_number),

    -- Composite unique targets so referrers can enforce workspace scope (and, for
    -- current_version_id, same-policy ownership) through composite foreign keys.
    CONSTRAINT workspace_policy_versions_workspace_id_id_key UNIQUE (workspace_id, id),
    CONSTRAINT workspace_policy_versions_workspace_policy_id_key UNIQUE (workspace_id, policy_id, id),

    -- Policy body (nvisy_schema::policy::PolicyDefinition as JSON: rules, labels,
    -- fallback, retention, `when` predicate). Stored XChaCha20-Poly1305 encrypted
    -- with the workspace-derived key.
    definition      BYTEA           NOT NULL,
    CONSTRAINT workspace_policy_versions_definition_size CHECK (length(definition) BETWEEN 1 AND 1048576),

    -- Definition-scoped metadata that versions with the content.
    metadata        JSONB           NOT NULL DEFAULT '{}',
    CONSTRAINT workspace_policy_versions_metadata_size CHECK (length(metadata::TEXT) BETWEEN 2 AND 65536),

    -- Creation timestamp (versions are immutable, so there is no updated_at).
    created_at      TIMESTAMPTZ     NOT NULL DEFAULT current_timestamp
);

-- A policy's versions, newest first.
CREATE INDEX workspace_policy_versions_policy_idx
    ON workspace_policy_versions (policy_id, version_number DESC);

COMMENT ON TABLE workspace_policy_versions IS 'Immutable snapshots of a policy''s definition; a detection pins the version it ran.';
COMMENT ON COLUMN workspace_policy_versions.id IS 'Unique version identifier';
COMMENT ON COLUMN workspace_policy_versions.policy_id IS 'Logical policy this is a version of';
COMMENT ON COLUMN workspace_policy_versions.workspace_id IS 'Owning workspace';
COMMENT ON COLUMN workspace_policy_versions.account_id IS 'Account that authored this version';
COMMENT ON COLUMN workspace_policy_versions.version_number IS 'Monotonic per-policy version number (1..N)';
COMMENT ON COLUMN workspace_policy_versions.definition IS 'Encrypted policy body (XChaCha20-Poly1305, workspace-derived key)';
COMMENT ON COLUMN workspace_policy_versions.metadata IS 'Definition-scoped metadata that versions with the content';
COMMENT ON COLUMN workspace_policy_versions.created_at IS 'When this version was created';

-- Now that the version table exists, point current_version_id at it with a
-- composite key so the live version must belong to this same policy (and
-- workspace), not merely be some existing version. current_version_id is
-- nullable, and MATCH SIMPLE skips the check while it is NULL (during create,
-- before the first version exists). A version is never deleted, so this never
-- cascades; the default (NO ACTION) is correct.
ALTER TABLE workspace_policies
    ADD CONSTRAINT workspace_policies_current_version_fkey
    FOREIGN KEY (workspace_id, id, current_version_id)
    REFERENCES workspace_policy_versions (workspace_id, policy_id, id);

-- Policy lifecycle events feed the activity log and webhooks.
ALTER TYPE ACTIVITY_TYPE ADD VALUE IF NOT EXISTS 'policy.created';
ALTER TYPE ACTIVITY_TYPE ADD VALUE IF NOT EXISTS 'policy.updated';
ALTER TYPE ACTIVITY_TYPE ADD VALUE IF NOT EXISTS 'policy.deleted';

ALTER TYPE WEBHOOK_EVENT ADD VALUE IF NOT EXISTS 'policy.created';
ALTER TYPE WEBHOOK_EVENT ADD VALUE IF NOT EXISTS 'policy.updated';
ALTER TYPE WEBHOOK_EVENT ADD VALUE IF NOT EXISTS 'policy.deleted';
