-- Activities: per-workspace audit log of member and resource actions.
-- Workspace-scoped but a standalone audit feature.

-- Enum types for workspace_activities table
CREATE TYPE ACTIVITY_TYPE AS ENUM (
    -- Workspace activities
    'workspace:created',
    'workspace:updated',
    'workspace:deleted',

    -- Member activities
    'member:added',
    'member:updated',
    'member:deleted',

    -- Invite activities
    'invite:created',
    'invite:accepted',
    'invite:declined',
    'invite:canceled',

    -- Connection activities
    'connection:created',
    'connection:updated',
    'connection:deleted',
    'connection:sync.completed',
    'connection:sync.failed',

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

    -- Pipeline activities
    'pipeline:created',
    'pipeline:updated',
    'pipeline:deleted',
    'pipeline:run.started',
    'pipeline:run.analyzed',
    'pipeline:run.completed',
    'pipeline:run.failed',

    -- Policy activities
    'policy:created',
    'policy:updated',
    'policy:deleted'
);

COMMENT ON TYPE ACTIVITY_TYPE IS
    'Defines the type of activity performed in a workspace for audit logging.';

-- Workspace activities table definition
CREATE TABLE workspace_activities (
    -- Primary identifier
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References
    workspace_id    UUID        NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    account_id    UUID        NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,

    -- Activity details. The type is the client-side localization key; the params
    -- carry its typed fields (an `activityType`-tagged payload). No rendered text
    -- is stored — the client renders the copy from the type and params.
    activity_type ACTIVITY_TYPE NOT NULL,
    params        JSONB         NOT NULL DEFAULT '{}',

    CONSTRAINT workspace_activities_params_size CHECK (length(params::TEXT) BETWEEN 2 AND 4096),

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
    ON workspace_activities (account_id, created_at DESC);

CREATE INDEX workspace_activities_activity_type_idx
    ON workspace_activities (activity_type, workspace_id, created_at DESC);

-- Comments for workspace_activities table
COMMENT ON TABLE workspace_activities IS
    'Comprehensive audit log for all workspace activities and changes.';

COMMENT ON COLUMN workspace_activities.id IS 'Unique activity log entry identifier (UUID)';
COMMENT ON COLUMN workspace_activities.workspace_id IS 'Reference to the workspace';
COMMENT ON COLUMN workspace_activities.account_id IS 'Account that performed the activity';
COMMENT ON COLUMN workspace_activities.activity_type IS 'Type of activity; the client-side localization key';
COMMENT ON COLUMN workspace_activities.params IS 'Typed params for the activity type (JSON, 2B-4KB)';
COMMENT ON COLUMN workspace_activities.ip_address IS 'IP address where activity originated';
COMMENT ON COLUMN workspace_activities.user_agent IS 'User agent of the client';
COMMENT ON COLUMN workspace_activities.created_at IS 'Timestamp when the activity occurred';

