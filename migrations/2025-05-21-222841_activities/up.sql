-- Activities: per-workspace audit log of member and resource actions. A
-- standalone, workspace-scoped audit feature. Each row records one action's type
-- and its typed params; the client renders the copy from those.

-- Type of activity recorded in a workspace audit log.
CREATE TYPE ACTIVITY_TYPE AS ENUM (
    -- Workspace activities
    'workspace.created',
    'workspace.updated',
    'workspace.deleted',

    -- Member activities
    'member.added',
    'member.updated',
    'member.deleted',

    -- Invite activities
    'invite.created',
    'invite.accepted',
    'invite.declined',
    'invite.canceled',

    -- Connection activities
    'connection.created',
    'connection.updated',
    'connection.deleted',
    'connection.sync.started',
    'connection.sync.completed',
    'connection.sync.failed',

    -- Webhook activities
    'webhook.created',
    'webhook.updated',
    'webhook.deleted',

    -- File activities
    'file.created',
    'file.updated',
    'file.deleted',

    -- Pipeline activities
    'pipeline.created',
    'pipeline.updated',
    'pipeline.deleted',
    'pipeline.run.started',
    'pipeline.run.analyzed',
    'pipeline.run.completed',
    'pipeline.run.failed',

    -- Policy activities
    'policy.created',
    'policy.updated',
    'policy.deleted'
);

COMMENT ON TYPE ACTIVITY_TYPE IS 'Type of activity performed in a workspace, for audit logging.';

-- Workspace activities table: one audit-log entry per member or resource action.
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

-- Most recent activities per workspace (the workspace audit feed).
CREATE INDEX workspace_activities_workspace_recent_idx
    ON workspace_activities (workspace_id, created_at DESC);

-- Most recent activities performed by an account.
CREATE INDEX workspace_activities_account_recent_idx
    ON workspace_activities (account_id, created_at DESC);

-- Filter a workspace's feed by activity type, most recent first.
CREATE INDEX workspace_activities_activity_type_idx
    ON workspace_activities (activity_type, workspace_id, created_at DESC);

COMMENT ON TABLE workspace_activities IS 'Audit log of member and resource actions within a workspace.';
COMMENT ON COLUMN workspace_activities.id IS 'Unique activity log entry identifier';
COMMENT ON COLUMN workspace_activities.workspace_id IS 'Workspace this activity belongs to';
COMMENT ON COLUMN workspace_activities.account_id IS 'Account that performed the activity';
COMMENT ON COLUMN workspace_activities.activity_type IS 'Type of activity; the client-side localization key';
COMMENT ON COLUMN workspace_activities.params IS 'Typed params for the activity type (JSON, 2B-4KB)';
COMMENT ON COLUMN workspace_activities.ip_address IS 'IP address where the activity originated';
COMMENT ON COLUMN workspace_activities.user_agent IS 'User agent of the client';
COMMENT ON COLUMN workspace_activities.created_at IS 'Timestamp when the activity occurred';
