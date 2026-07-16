-- This migration creates tables for workspaces, members, invites, and related functionality

-- Workspaces table definition
CREATE TABLE workspaces (
    -- Primary identifiers
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Workspace identity and branding
    display_name     TEXT             NOT NULL,
    slug             TEXT             NOT NULL,
    description      TEXT             DEFAULT NULL,
    avatar_url       TEXT             DEFAULT NULL,

    CONSTRAINT workspaces_display_name_length CHECK (length(trim(display_name)) BETWEEN 3 AND 32),
    CONSTRAINT workspaces_description_length_max CHECK (length(description) <= 2000),

    -- Human-readable URL identity. Mirrors the WorkspaceSlug newtype: lowercase
    -- alphanumeric with single internal dashes, 3-32 characters.
    CONSTRAINT workspaces_slug_key UNIQUE (slug),
    CONSTRAINT workspaces_slug_length CHECK (length(slug) BETWEEN 3 AND 32),
    CONSTRAINT workspaces_slug_format CHECK (slug ~ '^[a-z0-9]+(-[a-z0-9]+)*$'),

    -- Workspace settings
    require_approval BOOLEAN            NOT NULL DEFAULT TRUE,

    -- Tags and extended metadata
    tags             TEXT[]             NOT NULL DEFAULT '{}',
    metadata         JSONB              NOT NULL DEFAULT '{}',
    settings         JSONB              NOT NULL DEFAULT '{}',

    CONSTRAINT workspaces_tags_count_max CHECK (array_length(tags, 1) IS NULL OR array_length(tags, 1) <= 20),
    CONSTRAINT workspaces_metadata_size CHECK (length(metadata::TEXT) BETWEEN 2 AND 8192),
    CONSTRAINT workspaces_settings_size CHECK (length(settings::TEXT) BETWEEN 2 AND 8192),

    -- Audit and ownership
    created_by       UUID               NOT NULL REFERENCES accounts (id),

    -- Lifecycle timestamps
    created_at       TIMESTAMPTZ        NOT NULL DEFAULT current_timestamp,
    updated_at       TIMESTAMPTZ        NOT NULL DEFAULT current_timestamp,
    deleted_at       TIMESTAMPTZ        DEFAULT NULL,

    CONSTRAINT workspaces_updated_after_created CHECK (updated_at >= created_at),
    CONSTRAINT workspaces_deleted_after_updated CHECK (deleted_at IS NULL OR deleted_at >= updated_at),
    CONSTRAINT workspaces_deleted_after_created CHECK (deleted_at IS NULL OR deleted_at >= created_at)
);

-- Triggers for workspaces table
SELECT setup_updated_at('workspaces');

-- Indexes for workspaces table
CREATE UNIQUE INDEX workspaces_display_name_owner_unique_idx
    ON workspaces (lower(display_name), created_by)
    WHERE deleted_at IS NULL;

CREATE INDEX workspaces_active_lookup_idx
    ON workspaces (id)
    WHERE deleted_at IS NULL;

CREATE INDEX workspaces_owner_lookup_idx
    ON workspaces (created_by, created_at DESC)
    WHERE deleted_at IS NULL;

CREATE INDEX workspaces_tags_lookup_idx
    ON workspaces USING gin (tags)
    WHERE array_length(tags, 1) > 0 AND deleted_at IS NULL;

CREATE INDEX workspaces_metadata_lookup_idx
    ON workspaces USING gin (metadata)
    WHERE deleted_at IS NULL;

CREATE INDEX workspaces_display_name_trgm_idx
    ON workspaces USING gin (display_name gin_trgm_ops)
    WHERE deleted_at IS NULL;

-- Comments for workspaces table
COMMENT ON TABLE workspaces IS
    'Enhanced workspace management with comprehensive features, quotas, and security controls.';

COMMENT ON COLUMN workspaces.id IS 'Unique workspace identifier (UUID)';
COMMENT ON COLUMN workspaces.display_name IS 'Human-readable workspace name (3-32 characters)';
COMMENT ON COLUMN workspaces.description IS 'Detailed workspace description (up to 2000 characters)';
COMMENT ON COLUMN workspaces.avatar_url IS 'URL to workspace avatar/logo image';
COMMENT ON COLUMN workspaces.require_approval IS 'Require approval for new member requests';
COMMENT ON COLUMN workspaces.tags IS 'Array of tags for workspace classification and search';
COMMENT ON COLUMN workspaces.metadata IS 'Extended workspace metadata (JSON, 2B-8KB)';
COMMENT ON COLUMN workspaces.settings IS 'Workspace-specific settings and preferences (JSON, 2B-8KB)';
COMMENT ON COLUMN workspaces.created_by IS 'Account that created this workspace (becomes first owner)';
COMMENT ON COLUMN workspaces.created_at IS 'Timestamp when the workspace was created';
COMMENT ON COLUMN workspaces.updated_at IS 'Timestamp when the workspace was last modified (auto-updated)';
COMMENT ON COLUMN workspaces.deleted_at IS 'Timestamp when the workspace was soft-deleted (NULL if active)';

-- Enum types for workspace_members table
CREATE TYPE WORKSPACE_ROLE AS ENUM (
    'owner',        -- Full workspace ownership and management
    'admin',        -- Can manage members, connections, and settings
    'member',       -- Can edit content and manage files
    'guest'         -- Read-only access to workspace content
);

COMMENT ON TYPE WORKSPACE_ROLE IS
    'Defines granular access roles for workspace members with hierarchical permissions.';

-- Workspace members table definition
CREATE TABLE workspace_members (
    -- Primary keys (composite)
    workspace_id       UUID           NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    account_id         UUID           NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,

    PRIMARY KEY (workspace_id, account_id),

    -- Role
    member_role        WORKSPACE_ROLE NOT NULL DEFAULT 'guest',

    -- Notification preferences
    notify_via_email          BOOLEAN              NOT NULL DEFAULT FALSE,
    notification_events_app   NOTIFICATION_EVENT[] NOT NULL DEFAULT '{}',
    notification_events_email NOTIFICATION_EVENT[] NOT NULL DEFAULT '{}',

    -- Audit tracking
    created_by         UUID           NOT NULL REFERENCES accounts (id),
    updated_by         UUID           NOT NULL REFERENCES accounts (id),

    -- Lifecycle timestamps
    created_at         TIMESTAMPTZ    NOT NULL DEFAULT current_timestamp,
    updated_at         TIMESTAMPTZ    NOT NULL DEFAULT current_timestamp,

    CONSTRAINT workspace_members_updated_after_created CHECK (updated_at >= created_at)
);

-- Triggers for workspace_members table
SELECT setup_updated_at('workspace_members');

-- Indexes for workspace_members table
CREATE INDEX workspace_members_account_workspaces_idx
    ON workspace_members (account_id, created_at DESC);

CREATE INDEX workspace_members_workspace_role_idx
    ON workspace_members (workspace_id, member_role);

CREATE INDEX workspace_members_role_lookup_idx
    ON workspace_members (member_role, workspace_id);

-- Comments for workspace_members table
COMMENT ON TABLE workspace_members IS
    'Workspace membership with roles and notification preferences.';

COMMENT ON COLUMN workspace_members.workspace_id IS 'Reference to the workspace';
COMMENT ON COLUMN workspace_members.account_id IS 'Reference to the member account';
COMMENT ON COLUMN workspace_members.member_role IS 'Member role defining base permissions level';
COMMENT ON COLUMN workspace_members.notify_via_email IS 'Whether to send email notifications';
COMMENT ON COLUMN workspace_members.notification_events_app IS 'Notification events to receive in-app';
COMMENT ON COLUMN workspace_members.notification_events_email IS 'Notification events to receive via email';
COMMENT ON COLUMN workspace_members.created_by IS 'Account that added this member';
COMMENT ON COLUMN workspace_members.updated_by IS 'Account that last modified this membership';
COMMENT ON COLUMN workspace_members.created_at IS 'Timestamp when the membership was created';
COMMENT ON COLUMN workspace_members.updated_at IS 'Timestamp when the membership was last modified';

-- Enum types for workspace_invites table
CREATE TYPE INVITE_STATUS AS ENUM (
    'pending',      -- Invitation sent, awaiting response
    'accepted',     -- Invitation accepted, member added
    'declined',     -- Invitation declined by invitee
    'canceled',     -- Invitation canceled by inviter
    'expired',      -- Invitation expired due to timeout
    'revoked'       -- Invitation revoked by admin
);

COMMENT ON TYPE INVITE_STATUS IS
    'Comprehensive status tracking for workspace invitations.';

-- Workspace invites table definition
CREATE TABLE workspace_invites (
    -- Unique invite identifier
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References
    workspace_id   UUID            NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,

    -- Composite key target for workspace-scoped access and foreign keys.
    CONSTRAINT workspace_invites_workspace_id_id_key UNIQUE (workspace_id, id),

    -- Invitation details
    invitee_email  TEXT            DEFAULT NULL,
    invited_role   WORKSPACE_ROLE  NOT NULL DEFAULT 'guest',
    invite_token   TEXT            NOT NULL DEFAULT generate_secure_token(32),

    CONSTRAINT workspace_invites_invite_token_not_empty CHECK (trim(invite_token) <> ''),
    CONSTRAINT workspace_invites_invitee_email_format CHECK (invitee_email IS NULL OR is_valid_email(invitee_email)),

    -- Invite status and expiration
    invite_status  INVITE_STATUS NOT NULL DEFAULT 'pending',
    expires_at     TIMESTAMPTZ   NOT NULL DEFAULT current_timestamp + INTERVAL '7 days',
    responded_at   TIMESTAMPTZ   DEFAULT NULL,

    -- Audit tracking
    created_by     UUID          NOT NULL REFERENCES accounts (id),
    updated_by     UUID          NOT NULL REFERENCES accounts (id),

    -- Lifecycle timestamps
    created_at     TIMESTAMPTZ   NOT NULL DEFAULT current_timestamp,
    updated_at     TIMESTAMPTZ   NOT NULL DEFAULT current_timestamp,

    CONSTRAINT workspace_invites_expires_after_created CHECK (expires_at > created_at),
    CONSTRAINT workspace_invites_updated_after_created CHECK (updated_at >= created_at),
    CONSTRAINT workspace_invites_responded_after_created CHECK (responded_at IS NULL OR responded_at >= created_at)
);

-- Triggers for workspace_invites table
SELECT setup_updated_at('workspace_invites');

-- Indexes for workspace_invites table
CREATE INDEX workspace_invites_token_lookup_idx
    ON workspace_invites (invite_token)
    WHERE invite_status = 'pending';

CREATE INDEX workspace_invites_expiry_cleanup_idx
    ON workspace_invites (expires_at)
    WHERE invite_status = 'pending';

CREATE INDEX workspace_invites_invitee_lookup_idx
    ON workspace_invites (invitee_email, invite_status, created_at DESC)
    WHERE invitee_email IS NOT NULL;

-- Comments for workspace_invites table
COMMENT ON TABLE workspace_invites IS
    'Workspace invitations with comprehensive tracking and security features.';

COMMENT ON COLUMN workspace_invites.id IS 'Unique invite identifier (UUID)';
COMMENT ON COLUMN workspace_invites.workspace_id IS 'Reference to the workspace being invited to';
COMMENT ON COLUMN workspace_invites.invitee_email IS 'Email address of invitee (null for open invite codes)';
COMMENT ON COLUMN workspace_invites.invited_role IS 'Role that will be assigned upon acceptance';
COMMENT ON COLUMN workspace_invites.invite_token IS 'Secure token for invite validation';
COMMENT ON COLUMN workspace_invites.invite_status IS 'Current status of the invitation';
COMMENT ON COLUMN workspace_invites.expires_at IS 'Invitation expiration timestamp';
COMMENT ON COLUMN workspace_invites.responded_at IS 'Timestamp when invitee responded';
COMMENT ON COLUMN workspace_invites.created_by IS 'Account that sent the invitation';
COMMENT ON COLUMN workspace_invites.updated_by IS 'Account that last modified the invitation';
COMMENT ON COLUMN workspace_invites.created_at IS 'Timestamp when the invitation was created';
COMMENT ON COLUMN workspace_invites.updated_at IS 'Timestamp when the invitation was last modified';

-- Create workspace member summary view
CREATE VIEW workspace_member_summary AS
SELECT
    p.id                                                  AS workspace_id,
    p.display_name,
    COUNT(pm.account_id)                                  AS total_members,
    COUNT(CASE WHEN pm.member_role = 'owner' THEN 1 END)  AS owners,
    COUNT(CASE WHEN pm.member_role = 'admin' THEN 1 END)  AS admins,
    COUNT(CASE WHEN pm.member_role = 'member' THEN 1 END) AS members,
    COUNT(CASE WHEN pm.member_role = 'guest' THEN 1 END)  AS guests
FROM workspaces p
    LEFT JOIN workspace_members pm ON p.id = pm.workspace_id
WHERE p.deleted_at IS NULL
GROUP BY p.id, p.display_name;

COMMENT ON VIEW workspace_member_summary IS
    'Summary of workspace membership statistics.';

-- Create pending workspace invites view
CREATE VIEW pending_workspace_invites AS
SELECT
    pi.id,
    pi.workspace_id,
    p.display_name                      AS workspace_name,
    pi.invited_role,
    pi.created_by,
    creator.display_name                AS inviter_name,
    pi.created_at,
    pi.expires_at,
    EXTRACT(EPOCH FROM (pi.expires_at - CURRENT_TIMESTAMP)) / 86400 AS days_until_expiry
FROM workspace_invites pi
    JOIN workspaces p ON pi.workspace_id = p.id
    JOIN accounts creator ON pi.created_by = creator.id
WHERE pi.invite_status = 'pending'
    AND pi.expires_at > CURRENT_TIMESTAMP
    AND p.deleted_at IS NULL;

COMMENT ON VIEW pending_workspace_invites IS
    'Active workspace invitations with workspace and inviter details.';

-- Function to cleanup expired invites
CREATE OR REPLACE FUNCTION cleanup_expired_invites()
RETURNS INTEGER
LANGUAGE plpgsql AS $$
DECLARE
    _expired_count INTEGER;
BEGIN
    WITH expired_invites AS (
        UPDATE workspace_invites
        SET invite_status = 'expired',
            updated_by = created_by,
            updated_at = CURRENT_TIMESTAMP
        WHERE invite_status = 'pending'
            AND expires_at < CURRENT_TIMESTAMP
        RETURNING 1
    )
    SELECT COUNT(*) INTO _expired_count FROM expired_invites;

    RETURN _expired_count;
END;
$$;

COMMENT ON FUNCTION cleanup_expired_invites() IS
    'Marks expired workspace invitations as expired and returns count of updated records.';
