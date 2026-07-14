-- Activities: per-workspace audit log of member and resource actions.
-- Workspace-scoped but a standalone audit feature.

-- Enum types for workspace_activities table
CREATE TYPE ACTIVITY_TYPE AS ENUM (
    -- Workspace activities
    'workspace:created',
    'workspace:updated',
    'workspace:deleted',
    'workspace:exported',
    'workspace:imported',

    -- Member activities
    'member:deleted',
    'member:updated',

    -- Invite activities
    'invite:created',
    'invite:accepted',
    'invite:declined',
    'invite:canceled',

    -- Connection activities
    'connection:created',
    'connection:updated',
    'connection:deleted',
    'connection:synced',

    -- Webhook activities
    'webhook:created',
    'webhook:updated',
    'webhook:deleted',
    'webhook:triggered',

    -- File activities
    'file:created',
    'file:updated',
    'file:deleted',
    'file:verified',

    -- Custom activities
    'custom'
);

COMMENT ON TYPE ACTIVITY_TYPE IS
    'Defines the type of activity performed in a workspace for audit logging.';

-- Workspace activities table definition
CREATE TABLE workspace_activities (
    -- Primary identifier
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References
    workspace_id    UUID        NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    account_id    UUID        DEFAULT NULL REFERENCES accounts (id) ON DELETE SET NULL,

    -- Activity details
    activity_type ACTIVITY_TYPE NOT NULL,
    description   TEXT          NOT NULL DEFAULT '',
    metadata      JSONB         NOT NULL DEFAULT '{}',

    CONSTRAINT workspace_activities_description_length_max CHECK (length(description) <= 500),
    CONSTRAINT workspace_activities_metadata_size CHECK (length(metadata::TEXT) BETWEEN 2 AND 4096),

    -- Context tracking
    ip_address    INET        DEFAULT NULL,
    user_agent    TEXT        DEFAULT NULL,

    -- Lifecycle timestamp
    created_at    TIMESTAMPTZ NOT NULL DEFAULT current_timestamp
);

-- Indexes for workspace_activities table
CREATE INDEX workspace_activities_workspace_recent_idx
    ON workspace_activities (workspace_id, created_at DESC);

CREATE INDEX workspace_activities_account_recent_idx
    ON workspace_activities (account_id, created_at DESC)
    WHERE account_id IS NOT NULL;

CREATE INDEX workspace_activities_activity_type_idx
    ON workspace_activities (activity_type, workspace_id, created_at DESC);

-- Comments for workspace_activities table
COMMENT ON TABLE workspace_activities IS
    'Comprehensive audit log for all workspace activities and changes.';

COMMENT ON COLUMN workspace_activities.id IS 'Unique activity log entry identifier (UUID)';
COMMENT ON COLUMN workspace_activities.workspace_id IS 'Reference to the workspace';
COMMENT ON COLUMN workspace_activities.account_id IS 'Account that performed the activity (NULL for system)';
COMMENT ON COLUMN workspace_activities.activity_type IS 'Type of activity performed';
COMMENT ON COLUMN workspace_activities.description IS 'Human-readable description of the activity';
COMMENT ON COLUMN workspace_activities.metadata IS 'Additional activity context (JSON, 2B-4KB)';
COMMENT ON COLUMN workspace_activities.ip_address IS 'IP address where activity originated';
COMMENT ON COLUMN workspace_activities.user_agent IS 'User agent of the client';
COMMENT ON COLUMN workspace_activities.created_at IS 'Timestamp when the activity occurred';

