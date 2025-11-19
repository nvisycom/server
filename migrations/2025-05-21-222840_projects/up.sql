-- This migration creates tables for projects, members, invites, and related functionality

-- Create project status enum
CREATE TYPE PROJECT_STATUS AS ENUM (
    'active',       -- Project is active and accessible
    'archived',     -- Project is archived but accessible
    'suspended'     -- Project is temporarily suspended
);

COMMENT ON TYPE PROJECT_STATUS IS
    'Defines the operational status of projects in the system.';

-- Create project visibility enum
CREATE TYPE PROJECT_VISIBILITY AS ENUM (
    'private',      -- Only members can access
    'public'        -- Anyone can discover (read permissions still apply)
);

COMMENT ON TYPE PROJECT_VISIBILITY IS
    'Defines project visibility and discovery settings.';

-- Create comprehensive project role enum
CREATE TYPE PROJECT_ROLE AS ENUM (
    'owner',        -- Full control, can delete project and manage all aspects
    'admin',        -- Administrative access, cannot delete project
    'editor',       -- Can edit content and manage files
    'viewer'        -- Read-only access to project content
);

COMMENT ON TYPE PROJECT_ROLE IS
    'Defines granular access roles for project members with hierarchical permissions.';

-- Create project invite status enum
CREATE TYPE INVITE_STATUS AS ENUM (
    'pending',      -- Invitation sent, awaiting response
    'accepted',     -- Invitation accepted, member added
    'declined',     -- Invitation declined by invitee
    'canceled',     -- Invitation canceled by inviter
    'expired',      -- Invitation expired due to timeout
    'revoked'       -- Invitation revoked by admin
);

COMMENT ON TYPE INVITE_STATUS IS
    'Comprehensive status tracking for project invitations.';

-- Create integration status enum
CREATE TYPE INTEGRATION_STATUS AS ENUM (
    'pending',      -- Integration is being set up
    'executing',    -- Integration is actively running
    'failure'       -- Integration has failed
);

COMMENT ON TYPE INTEGRATION_STATUS IS
    'Defines the operational status of project integrations.';

-- Create integration type enum
CREATE TYPE INTEGRATION_TYPE AS ENUM (
    'webhook',      -- Generic webhook integration
    'storage',      -- External storage integration (S3, etc.)
    'other'         -- Other integration types
);

COMMENT ON TYPE INTEGRATION_TYPE IS
    'Defines the type/category of project integrations.';

-- Create activity type enum
CREATE TYPE ACTIVITY_TYPE AS ENUM (
    -- Project activities
    'project_created',
    'project_updated',
    'project_deleted',
    'project_archived',
    'project_restored',
    'project_settings_changed',
    'project_exported',
    'project_imported',

    -- Member activities
    'member_added',
    'member_kicked',
    'member_updated',
    'member_invited',
    'member_invite_accepted',
    'member_invite_declined',
    'member_invite_canceled',

    -- Integration activities
    'integration_created',
    'integration_updated',
    'integration_deleted',
    'integration_enabled',
    'integration_disabled',
    'integration_synced',
    'integration_succeeded',
    'integration_failed',

    -- Document activities
    'document_created',
    'document_updated',
    'document_deleted',
    'document_processed',
    'document_uploaded',
    'document_downloaded',
    'document_verified',

    -- Comment activities
    'comment_added',
    'comment_updated',
    'comment_deleted',

    -- Custom activities
    'custom'
);

COMMENT ON TYPE ACTIVITY_TYPE IS
    'Defines the type of activity performed in a project for audit logging.';

-- Create enhanced projects table
CREATE TABLE projects (
    -- Primary identifiers
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Project identity and branding
    display_name     TEXT             NOT NULL,
    description      TEXT             DEFAULT NULL,
    avatar_url       TEXT             DEFAULT NULL,

    CONSTRAINT projects_display_name_length CHECK (length(trim(display_name)) BETWEEN 3 AND 32),
    CONSTRAINT projects_description_length_max CHECK (length(description) <= 2000),

    -- Project status and visibility
    status           PROJECT_STATUS     NOT NULL DEFAULT 'active',
    visibility       PROJECT_VISIBILITY NOT NULL DEFAULT 'private',

    -- Data retention and cleanup
    keep_for_sec     INTEGER            NOT NULL DEFAULT 604800,
    auto_cleanup     BOOLEAN            NOT NULL DEFAULT TRUE,

    CONSTRAINT projects_keep_for_sec_range CHECK (keep_for_sec BETWEEN 3600 AND 31536000),

    -- Resource limits and quotas
    max_members      INTEGER            DEFAULT NULL,
    max_storage      INTEGER            DEFAULT NULL,

    CONSTRAINT projects_max_members_min CHECK (max_members IS NULL OR max_members >= 1),
    CONSTRAINT projects_max_members_max CHECK (max_members IS NULL OR max_members <= 1000),
    CONSTRAINT projects_max_storage_min CHECK (max_storage IS NULL OR max_storage >= 1),

    -- Project settings
    require_approval BOOLEAN            NOT NULL DEFAULT TRUE,
    enable_comments  BOOLEAN            NOT NULL DEFAULT TRUE,

    -- Tags and extended metadata
    tags             TEXT[]             NOT NULL DEFAULT '{}',
    metadata         JSONB              NOT NULL DEFAULT '{}',
    settings         JSONB              NOT NULL DEFAULT '{}',

    CONSTRAINT projects_tags_count_max CHECK (array_length(tags, 1) IS NULL OR array_length(tags, 1) <= 20),
    CONSTRAINT projects_metadata_size CHECK (length(metadata::TEXT) BETWEEN 2 AND 8192),
    CONSTRAINT projects_settings_size CHECK (length(settings::TEXT) BETWEEN 2 AND 8192),

    -- Audit and ownership
    created_by       UUID               NOT NULL REFERENCES accounts (id),

    -- Lifecycle timestamps
    created_at       TIMESTAMPTZ        NOT NULL DEFAULT current_timestamp,
    updated_at       TIMESTAMPTZ        NOT NULL DEFAULT current_timestamp,
    archived_at      TIMESTAMPTZ        DEFAULT NULL,
    deleted_at       TIMESTAMPTZ        DEFAULT NULL,

    CONSTRAINT projects_updated_after_created CHECK (updated_at >= created_at),
    CONSTRAINT projects_deleted_after_updated CHECK (deleted_at IS NULL OR deleted_at >= updated_at),
    CONSTRAINT projects_archived_after_created CHECK (archived_at IS NULL OR archived_at >= created_at),
    CONSTRAINT projects_deleted_after_created CHECK (deleted_at IS NULL OR deleted_at >= created_at),
    CONSTRAINT projects_deleted_after_archived CHECK (deleted_at IS NULL OR archived_at IS NULL OR deleted_at >= archived_at),

    -- Business logic constraints
    CONSTRAINT projects_active_status_not_archived CHECK (NOT (status = 'active' AND archived_at IS NOT NULL)),
    CONSTRAINT projects_archive_status_consistency CHECK ((archived_at IS NULL) = (status != 'archived'))
);

-- Set up automatic updated_at trigger
SELECT setup_updated_at('projects');

-- Create comprehensive indexes for projects
CREATE UNIQUE INDEX projects_display_name_owner_unique_idx
    ON projects (lower(display_name), created_by)
    WHERE deleted_at IS NULL;

CREATE INDEX projects_active_lookup_idx
    ON projects (id, status, visibility)
    WHERE deleted_at IS NULL;

CREATE INDEX projects_owner_lookup_idx
    ON projects (created_by, status, created_at DESC)
    WHERE deleted_at IS NULL;

CREATE INDEX projects_visibility_lookup_idx
    ON projects (visibility, status, updated_at DESC)
    WHERE deleted_at IS NULL AND visibility = 'public';

CREATE INDEX projects_tags_lookup_idx
    ON projects USING gin (tags)
    WHERE array_length(tags, 1) > 0 AND deleted_at IS NULL;

CREATE INDEX projects_cleanup_idx
    ON projects (created_at, keep_for_sec, auto_cleanup)
    WHERE auto_cleanup = TRUE AND deleted_at IS NULL;

CREATE INDEX projects_metadata_lookup_idx
    ON projects USING gin (metadata)
    WHERE deleted_at IS NULL;

-- Add comprehensive table and column comments
COMMENT ON TABLE projects IS
    'Enhanced project management with comprehensive features, quotas, and security controls.';

COMMENT ON COLUMN projects.id IS 'Unique project identifier (UUID)';
COMMENT ON COLUMN projects.display_name IS 'Human-readable project name (3-32 characters)';
COMMENT ON COLUMN projects.description IS 'Detailed project description (up to 2000 characters)';
COMMENT ON COLUMN projects.avatar_url IS 'URL to project avatar/logo image';
COMMENT ON COLUMN projects.status IS 'Current operational status of the project';
COMMENT ON COLUMN projects.visibility IS 'Project visibility and discovery settings';
COMMENT ON COLUMN projects.keep_for_sec IS 'Data retention period in seconds (1 hour to 1 year)';
COMMENT ON COLUMN projects.auto_cleanup IS 'Enable automatic cleanup of old project data';
COMMENT ON COLUMN projects.max_members IS 'Maximum number of members allowed (NULL = unlimited)';
COMMENT ON COLUMN projects.max_storage IS 'Maximum storage in megabytes (NULL = unlimited)';
COMMENT ON COLUMN projects.require_approval IS 'Require approval for new member requests';
COMMENT ON COLUMN projects.enable_comments IS 'Enable commenting features within the project';
COMMENT ON COLUMN projects.tags IS 'Array of tags for project classification and search';
COMMENT ON COLUMN projects.metadata IS 'Extended project metadata (JSON, 2B-8KB)';
COMMENT ON COLUMN projects.settings IS 'Project-specific settings and preferences (JSON, 2B-8KB)';
COMMENT ON COLUMN projects.created_by IS 'Account that created this project (becomes first owner)';
COMMENT ON COLUMN projects.created_at IS 'Timestamp when the project was created';
COMMENT ON COLUMN projects.updated_at IS 'Timestamp when the project was last modified (auto-updated)';
COMMENT ON COLUMN projects.archived_at IS 'Timestamp when the project was archived';
COMMENT ON COLUMN projects.deleted_at IS 'Timestamp when the project was soft-deleted (NULL if active)';

-- Create enhanced project members table
CREATE TABLE project_members (
    -- Primary keys (composite)
    project_id         UUID         NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    account_id         UUID         NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,

    PRIMARY KEY (project_id, account_id),

    -- Role and permissions
    member_role        PROJECT_ROLE NOT NULL DEFAULT 'viewer',
    custom_permissions JSONB        NOT NULL DEFAULT '{}',

    CONSTRAINT project_members_custom_permissions_size CHECK (length(custom_permissions::TEXT) BETWEEN 2 AND 2048),

    -- Member preferences and settings
    show_order         INTEGER      NOT NULL DEFAULT 0,
    is_favorite        BOOLEAN      NOT NULL DEFAULT FALSE,
    is_hidden          BOOLEAN      NOT NULL DEFAULT FALSE,

    CONSTRAINT project_members_show_order_range CHECK (show_order BETWEEN -1000 AND 1000),

    -- Notification preferences
    notify_updates     BOOLEAN      NOT NULL DEFAULT TRUE,
    notify_comments    BOOLEAN      NOT NULL DEFAULT TRUE,
    notify_mentions    BOOLEAN      NOT NULL DEFAULT TRUE,

    -- Member status
    is_active          BOOLEAN      NOT NULL DEFAULT TRUE,
    last_accessed_at   TIMESTAMPTZ  DEFAULT NULL,

    -- Audit tracking
    created_by         UUID         NOT NULL REFERENCES accounts (id),
    updated_by         UUID         NOT NULL REFERENCES accounts (id),

    -- Lifecycle timestamps
    created_at         TIMESTAMPTZ  NOT NULL DEFAULT current_timestamp,
    updated_at         TIMESTAMPTZ  NOT NULL DEFAULT current_timestamp,

    CONSTRAINT project_members_updated_after_created CHECK (updated_at >= created_at),
    CONSTRAINT project_members_last_accessed_after_created CHECK (last_accessed_at IS NULL OR last_accessed_at >= created_at)
);

-- Set up automatic updated_at trigger
SELECT setup_updated_at('project_members');

-- Create indexes for project members
CREATE INDEX project_members_account_projects_idx
    ON project_members (account_id, is_active, show_order)
    WHERE is_active = TRUE;

CREATE INDEX project_members_project_active_idx
    ON project_members (project_id, member_role, is_active)
    WHERE is_active = TRUE;

CREATE INDEX project_members_role_lookup_idx
    ON project_members (member_role, project_id)
    WHERE is_active = TRUE;

CREATE INDEX project_members_activity_tracking_idx
    ON project_members (last_accessed_at DESC)
    WHERE last_accessed_at IS NOT NULL;

CREATE INDEX project_members_favorites_idx
    ON project_members (account_id, is_favorite, updated_at DESC)
    WHERE is_favorite = TRUE;

-- Add comprehensive table and column comments
COMMENT ON TABLE project_members IS
    'Project membership with enhanced roles, permissions, and preferences.';

COMMENT ON COLUMN project_members.project_id IS 'Reference to the project';
COMMENT ON COLUMN project_members.account_id IS 'Reference to the member account';
COMMENT ON COLUMN project_members.member_role IS 'Member role defining base permissions level';
COMMENT ON COLUMN project_members.custom_permissions IS 'Custom permission overrides (JSON, 2B-2KB)';
COMMENT ON COLUMN project_members.show_order IS 'Custom sort order for member project list (-1000 to 1000)';
COMMENT ON COLUMN project_members.is_favorite IS 'Mark project as favorite for quick access';
COMMENT ON COLUMN project_members.is_hidden IS 'Hide project from member project list';
COMMENT ON COLUMN project_members.notify_updates IS 'Receive notifications for project updates';
COMMENT ON COLUMN project_members.notify_comments IS 'Receive notifications for new comments';
COMMENT ON COLUMN project_members.notify_mentions IS 'Receive notifications when mentioned';
COMMENT ON COLUMN project_members.is_active IS 'Member status (inactive members retain access but are hidden)';
COMMENT ON COLUMN project_members.last_accessed_at IS 'Timestamp of member last project access';
COMMENT ON COLUMN project_members.created_by IS 'Account that added this member';
COMMENT ON COLUMN project_members.updated_by IS 'Account that last modified this membership';
COMMENT ON COLUMN project_members.created_at IS 'Timestamp when the membership was created';
COMMENT ON COLUMN project_members.updated_at IS 'Timestamp when the membership was last modified';

-- Create enhanced project invites table
CREATE TABLE project_invites (
    -- Unique invite identifier
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References
    project_id     UUID          NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    invitee_id     UUID          DEFAULT NULL REFERENCES accounts (id) ON DELETE SET NULL,

    -- Invitation details
    invited_role   PROJECT_ROLE  NOT NULL DEFAULT 'viewer',
    invite_message TEXT          NOT NULL DEFAULT '',
    invite_token   TEXT          NOT NULL DEFAULT generate_secure_token(32),

    CONSTRAINT project_invites_invite_message_length_max CHECK (length(invite_message) <= 1000),
    CONSTRAINT project_invites_invite_token_not_empty CHECK (trim(invite_token) <> ''),

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

    CONSTRAINT project_invites_expires_after_created CHECK (expires_at > created_at),
    CONSTRAINT project_invites_updated_after_created CHECK (updated_at >= created_at),
    CONSTRAINT project_invites_responded_after_created CHECK (responded_at IS NULL OR responded_at >= created_at)
);

-- Set up automatic updated_at trigger
SELECT setup_updated_at('project_invites');

-- Create indexes for project invites
CREATE INDEX project_invites_token_lookup_idx
    ON project_invites (invite_token)
    WHERE invite_status = 'pending';

CREATE INDEX project_invites_expiry_cleanup_idx
    ON project_invites (expires_at)
    WHERE invite_status = 'pending';

CREATE INDEX project_invites_invitee_lookup_idx
    ON project_invites (invitee_id, invite_status, created_at DESC)
    WHERE invitee_id IS NOT NULL;

-- Add comprehensive table and column comments
COMMENT ON TABLE project_invites IS
    'Project invitations with comprehensive tracking and security features.';

COMMENT ON COLUMN project_invites.id IS 'Unique invite identifier (UUID)';
COMMENT ON COLUMN project_invites.project_id IS 'Reference to the project being invited to';
COMMENT ON COLUMN project_invites.invitee_id IS 'Reference to invitee account (if exists)';
COMMENT ON COLUMN project_invites.invited_role IS 'Role that will be assigned upon acceptance';
COMMENT ON COLUMN project_invites.invite_message IS 'Custom message from inviter (up to 1000 chars)';
COMMENT ON COLUMN project_invites.invite_token IS 'Secure token for invite validation';
COMMENT ON COLUMN project_invites.invite_status IS 'Current status of the invitation';
COMMENT ON COLUMN project_invites.expires_at IS 'Invitation expiration timestamp';
COMMENT ON COLUMN project_invites.responded_at IS 'Timestamp when invitee responded';
COMMENT ON COLUMN project_invites.created_by IS 'Account that sent the invitation';
COMMENT ON COLUMN project_invites.updated_by IS 'Account that last modified the invitation';
COMMENT ON COLUMN project_invites.created_at IS 'Timestamp when the invitation was created';
COMMENT ON COLUMN project_invites.updated_at IS 'Timestamp when the invitation was last modified';

-- Create project activity log table
CREATE TABLE project_activities (
    -- Primary identifier
    id            BIGSERIAL PRIMARY KEY,

    -- References
    project_id    UUID        NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    account_id    UUID        DEFAULT NULL REFERENCES accounts (id) ON DELETE SET NULL,

    -- Activity details
    activity_type ACTIVITY_TYPE NOT NULL,
    description   TEXT          NOT NULL DEFAULT '',
    metadata      JSONB         NOT NULL DEFAULT '{}',

    CONSTRAINT project_activities_description_length_max CHECK (length(description) <= 500),
    CONSTRAINT project_activities_metadata_size CHECK (length(metadata::TEXT) BETWEEN 2 AND 4096),

    -- Context tracking
    ip_address    INET        DEFAULT NULL,
    user_agent    TEXT        DEFAULT NULL,

    -- Lifecycle timestamp
    created_at    TIMESTAMPTZ NOT NULL DEFAULT current_timestamp
);

-- Create indexes for activity log
CREATE INDEX project_activities_project_recent_idx
    ON project_activities (project_id, created_at DESC);

CREATE INDEX project_activities_account_recent_idx
    ON project_activities (account_id, created_at DESC)
    WHERE account_id IS NOT NULL;

CREATE INDEX project_activities_activity_type_idx
    ON project_activities (activity_type, project_id, created_at DESC);

-- Add table and column comments
COMMENT ON TABLE project_activities IS
    'Comprehensive audit log for all project activities and changes.';

COMMENT ON COLUMN project_activities.id IS 'Unique activity log entry identifier';
COMMENT ON COLUMN project_activities.project_id IS 'Reference to the project';
COMMENT ON COLUMN project_activities.account_id IS 'Account that performed the activity (NULL for system)';
COMMENT ON COLUMN project_activities.activity_type IS 'Type of activity performed';
COMMENT ON COLUMN project_activities.description IS 'Human-readable description of the activity';
COMMENT ON COLUMN project_activities.metadata IS 'Additional activity context (JSON, 2B-4KB)';
COMMENT ON COLUMN project_activities.ip_address IS 'IP address where activity originated';
COMMENT ON COLUMN project_activities.user_agent IS 'User agent of the client';
COMMENT ON COLUMN project_activities.created_at IS 'Timestamp when the activity occurred';

-- Create project integrations table
CREATE TABLE project_integrations (
    -- Primary identifier
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Reference
    project_id       UUID             NOT NULL REFERENCES projects (id) ON DELETE CASCADE,

    -- Integration details
    integration_name TEXT             NOT NULL,
    description      TEXT             NOT NULL DEFAULT '',
    integration_type INTEGRATION_TYPE NOT NULL,

    CONSTRAINT project_integrations_integration_name_not_empty CHECK (trim(integration_name) <> ''),
    CONSTRAINT project_integrations_description_length_max CHECK (length(description) <= 500),

    -- Configuration and credentials
    metadata         JSONB            NOT NULL DEFAULT '{}',
    credentials      JSONB            NOT NULL DEFAULT '{}',

    CONSTRAINT project_integrations_metadata_size CHECK (length(metadata::TEXT) BETWEEN 2 AND 8192),
    CONSTRAINT project_integrations_credentials_size CHECK (length(credentials::TEXT) BETWEEN 2 AND 4096),

    -- Integration status
    is_active        BOOLEAN          NOT NULL DEFAULT TRUE,
    last_sync_at     TIMESTAMPTZ      DEFAULT NULL,
    sync_status      INTEGRATION_STATUS DEFAULT 'pending',

    -- Audit tracking
    created_by       UUID             NOT NULL REFERENCES accounts (id),

    -- Lifecycle timestamps
    created_at       TIMESTAMPTZ      NOT NULL DEFAULT current_timestamp,
    updated_at       TIMESTAMPTZ      NOT NULL DEFAULT current_timestamp,

    CONSTRAINT project_integrations_updated_after_created CHECK (updated_at >= created_at),
    CONSTRAINT project_integrations_last_sync_after_created CHECK (last_sync_at IS NULL OR last_sync_at >= created_at)
);

-- Set up automatic updated_at trigger
SELECT setup_updated_at('project_integrations');

-- Create indexes for integrations
CREATE INDEX project_integrations_project_active_idx
    ON project_integrations (project_id, is_active, integration_type);

CREATE INDEX project_integrations_sync_status_idx
    ON project_integrations (sync_status, last_sync_at)
    WHERE is_active = TRUE;

-- Add table and column comments
COMMENT ON TABLE project_integrations IS
    'External service integrations for projects with configuration and sync tracking.';

COMMENT ON COLUMN project_integrations.id IS 'Unique integration identifier';
COMMENT ON COLUMN project_integrations.project_id IS 'Reference to the project';
COMMENT ON COLUMN project_integrations.integration_name IS 'Human-readable integration name';
COMMENT ON COLUMN project_integrations.description IS 'Integration description (up to 500 chars)';
COMMENT ON COLUMN project_integrations.integration_type IS 'Type/category of integration';
COMMENT ON COLUMN project_integrations.metadata IS 'Integration configuration and metadata (JSON, 2B-8KB)';
COMMENT ON COLUMN project_integrations.credentials IS 'Encrypted credentials (JSON, 2B-4KB)';
COMMENT ON COLUMN project_integrations.is_active IS 'Integration active status';
COMMENT ON COLUMN project_integrations.last_sync_at IS 'Timestamp of last synchronization';
COMMENT ON COLUMN project_integrations.sync_status IS 'Current integration status (pending, executing, failure)';
COMMENT ON COLUMN project_integrations.created_by IS 'Account that created the integration';
COMMENT ON COLUMN project_integrations.created_at IS 'Timestamp when integration was created';
COMMENT ON COLUMN project_integrations.updated_at IS 'Timestamp when integration was last modified';

-- Create project member summary view
CREATE VIEW project_member_summary AS
SELECT
    p.id                                                  AS project_id,
    p.display_name,
    p.status                                              AS project_status,
    COUNT(pm.account_id)                                  AS total_members,
    COUNT(CASE WHEN pm.member_role = 'owner' THEN 1 END)  AS owners,
    COUNT(CASE WHEN pm.member_role = 'admin' THEN 1 END)  AS admins,
    COUNT(CASE WHEN pm.member_role = 'editor' THEN 1 END) AS editors,
    COUNT(CASE WHEN pm.member_role = 'viewer' THEN 1 END) AS viewers,
    COUNT(CASE WHEN pm.is_active = FALSE THEN 1 END)      AS inactive_members,
    MAX(pm.last_accessed_at)                              AS last_member_access
FROM projects p
    LEFT JOIN project_members pm ON p.id = pm.project_id
WHERE p.deleted_at IS NULL
GROUP BY p.id, p.display_name, p.status;

COMMENT ON VIEW project_member_summary IS
    'Summary of project membership statistics and activity.';

-- Create pending project invites view
CREATE VIEW pending_project_invites AS
SELECT
    pi.id,
    pi.project_id,
    p.display_name                      AS project_name,
    pi.invited_role,
    pi.invite_message,
    pi.created_by,
    creator.display_name                AS inviter_name,
    pi.created_at,
    pi.expires_at,
    EXTRACT(EPOCH FROM (pi.expires_at - CURRENT_TIMESTAMP)) / 86400 AS days_until_expiry
FROM project_invites pi
    JOIN projects p ON pi.project_id = p.id
    JOIN accounts creator ON pi.created_by = creator.id
WHERE pi.invite_status = 'pending'
    AND pi.expires_at > CURRENT_TIMESTAMP
    AND p.deleted_at IS NULL;

COMMENT ON VIEW pending_project_invites IS
    'Active project invitations with project and inviter details.';

-- Function to check if user has specific permission on project
CREATE OR REPLACE FUNCTION check_project_permission(
    _project_id UUID,
    _account_id UUID,
    _required_permission TEXT
) RETURNS BOOLEAN
LANGUAGE plpgsql AS $$
DECLARE
    _member_role    PROJECT_ROLE;
    _custom_perms   JSONB;
    _has_permission BOOLEAN := FALSE;
BEGIN
    -- Get member role and custom permissions
    SELECT member_role, custom_permissions
    INTO _member_role, _custom_perms
    FROM project_members
    WHERE project_id = _project_id
        AND account_id = _account_id
        AND is_active = TRUE;

    -- If not a member, no permissions
    IF _member_role IS NULL THEN
        RETURN FALSE;
    END IF;

    -- Check role-based permissions
    _has_permission := CASE _required_permission
        WHEN 'read' THEN _member_role IN ('owner', 'admin', 'editor', 'viewer')
        WHEN 'write' THEN _member_role IN ('owner', 'admin', 'editor')
        WHEN 'admin' THEN _member_role IN ('owner', 'admin')
        WHEN 'owner' THEN _member_role = 'owner'
        ELSE FALSE
    END;

    -- Check custom permissions override
    IF NOT _has_permission AND _custom_perms ? _required_permission THEN
        _has_permission := (_custom_perms ->> _required_permission)::BOOLEAN;
    END IF;

    RETURN _has_permission;
END;
$$;

COMMENT ON FUNCTION check_project_permission(UUID, UUID, TEXT) IS
    'Checks if a user has specific permission on a project considering role and custom permissions.';

-- Function to cleanup expired invites
CREATE OR REPLACE FUNCTION cleanup_expired_invites()
RETURNS INTEGER
LANGUAGE plpgsql AS $$
DECLARE
    _expired_count INTEGER;
BEGIN
    WITH expired_invites AS (
        UPDATE project_invites
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
    'Marks expired project invitations as expired and returns count of updated records.';
