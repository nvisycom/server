-- This migration creates tables for projects, members, invites, and related functionality

-- Create project status enum
CREATE TYPE PROJECT_STATUS AS ENUM (
    'active', -- Project is active and accessible
    'archived', -- Project is archived but accessible
    'suspended', -- Project is temporarily suspended
    'template' -- Project serves as a template for new projects
    );

COMMENT ON TYPE PROJECT_STATUS IS
    'Defines the operational status of projects in the system.';

-- Create project visibility enum
CREATE TYPE PROJECT_VISIBILITY AS ENUM (
    'private', -- Only members can access
    'public' -- Anyone can discover (read permissions still apply)
    );

COMMENT ON TYPE PROJECT_VISIBILITY IS
    'Defines project visibility and discovery settings.';

-- Create comprehensive project role enum
CREATE TYPE PROJECT_ROLE AS ENUM (
    'owner', -- Full control, can delete project and manage all aspects
    'admin', -- Administrative access, cannot delete project
    'editor', -- Can edit content and manage files
    'viewer' -- Read-only access to project content
    );

COMMENT ON TYPE PROJECT_ROLE IS
    'Defines granular access roles for project members with hierarchical permissions.';

-- Create project invite status enum
CREATE TYPE INVITE_STATUS AS ENUM (
    'pending', -- Invitation sent, awaiting response
    'accepted', -- Invitation accepted, member added
    'declined', -- Invitation declined by invitee
    'canceled', -- Invitation canceled by inviter
    'expired', -- Invitation expired due to timeout
    'revoked' -- Invitation revoked by admin
    );

COMMENT ON TYPE INVITE_STATUS IS
    'Comprehensive status tracking for project invitations.';

-- Create enhanced projects table
CREATE TABLE projects
(
    -- Primary identifiers
    id               UUID PRIMARY KEY            DEFAULT gen_random_uuid(),

    -- Project identity and branding
    display_name     TEXT               NOT NULL,
    description      TEXT               NOT NULL DEFAULT '',
    avatar_url       TEXT                        DEFAULT NULL,

    CONSTRAINT projects_display_name_length_min CHECK (length(trim(display_name)) >= 3),
    CONSTRAINT projects_display_name_length_max CHECK (length(trim(display_name)) <= 32),
    CONSTRAINT projects_description_length_max CHECK (length(description) <= 2000),

    -- Project status and visibility
    status           PROJECT_STATUS     NOT NULL DEFAULT 'active',
    visibility       PROJECT_VISIBILITY NOT NULL DEFAULT 'private',

    -- Data retention and cleanup
    keep_for_sec     INTEGER            NOT NULL DEFAULT 604800,
    auto_cleanup     BOOLEAN            NOT NULL DEFAULT TRUE,

    CONSTRAINT projects_keep_for_sec_min CHECK (keep_for_sec >= 3600),
    CONSTRAINT projects_keep_for_sec_max CHECK (keep_for_sec <= 31536000),

    -- Resource limits and quotas
    max_members      INTEGER                     DEFAULT NULL,
    max_storage      INTEGER                     DEFAULT NULL,

    CONSTRAINT projects_max_members_min CHECK (max_members IS NULL OR max_members >= 1),
    CONSTRAINT projects_max_members_max CHECK (max_members IS NULL OR max_members <= 1000),
    CONSTRAINT projects_max_storage_min CHECK (max_storage IS NULL OR max_storage >= 1),

    -- Project settings
    require_approval BOOLEAN            NOT NULL DEFAULT TRUE,
    enable_comments  BOOLEAN            NOT NULL DEFAULT TRUE,

    -- Tags and extended metadata
    tags             TEXT[]             NOT NULL DEFAULT '{}',
    metadata         JSONB              NOT NULL DEFAULT '{}'::JSONB,
    settings         JSONB              NOT NULL DEFAULT '{}'::JSONB,

    CONSTRAINT projects_tags_count_max CHECK (array_length(tags, 1) IS NULL OR array_length(tags, 1) <= 20),
    CONSTRAINT projects_metadata_size_min CHECK (length(metadata::TEXT) >= 2),
    CONSTRAINT projects_metadata_size_max CHECK (length(metadata::TEXT) <= 8192),
    CONSTRAINT projects_settings_size_min CHECK (length(settings::TEXT) >= 2),
    CONSTRAINT projects_settings_size_max CHECK (length(settings::TEXT) <= 8192),

    -- Audit and ownership
    created_by       UUID               NOT NULL REFERENCES accounts (id),

    -- Lifecycle timestamps
    created_at       TIMESTAMPTZ        NOT NULL DEFAULT current_timestamp,
    updated_at       TIMESTAMPTZ        NOT NULL DEFAULT current_timestamp,
    archived_at      TIMESTAMPTZ                 DEFAULT NULL,
    deleted_at       TIMESTAMPTZ                 DEFAULT NULL,

    -- Chronological integrity constraints
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
    'Enhanced project management with comprehensive features, templates, quotas, and security controls.';

-- Primary identifiers
COMMENT ON COLUMN projects.id IS
    'Unique project identifier (UUID).';

-- Project identity and branding
COMMENT ON COLUMN projects.display_name IS
    'Human-readable project name (3-100 characters).';
COMMENT ON COLUMN projects.description IS
    'Detailed project description (up to 200 characters).';
COMMENT ON COLUMN projects.avatar_url IS
    'URL to project avatar/logo image.';

-- Project status and visibility
COMMENT ON COLUMN projects.status IS
    'Current operational status of the project.';
COMMENT ON COLUMN projects.visibility IS
    'Project visibility and discovery settings.';

-- Data retention and cleanup
COMMENT ON COLUMN projects.keep_for_sec IS
    'Data retention period in seconds (1 hour to 1 year).';
COMMENT ON COLUMN projects.auto_cleanup IS
    'Enable automatic cleanup of old project data.';

-- Resource limits and quotas
COMMENT ON COLUMN projects.max_members IS
    'Maximum number of members allowed (NULL = unlimited).';
COMMENT ON COLUMN projects.max_storage IS
    'Maximum storage in megabytes (NULL = unlimited).';

-- Project settings
COMMENT ON COLUMN projects.require_approval IS
    'Require approval for new member requests.';
COMMENT ON COLUMN projects.enable_comments IS
    'Enable commenting features within the project.';

-- Tags and extended metadata
COMMENT ON COLUMN projects.tags IS
    'Array of tags for project classification and search.';
COMMENT ON COLUMN projects.metadata IS
    'Extended project metadata (JSON, 2B-8KB).';
COMMENT ON COLUMN projects.settings IS
    'Project-specific settings and preferences (JSON, 2B-8KB).';

-- Audit and ownership
COMMENT ON COLUMN projects.created_by IS
    'Account that created this project (becomes first owner).';

-- Lifecycle timestamps
COMMENT ON COLUMN projects.created_at IS
    'Timestamp when the project was created.';
COMMENT ON COLUMN projects.updated_at IS
    'Timestamp when the project was last modified (auto-updated).';
COMMENT ON COLUMN projects.archived_at IS
    'Timestamp when the project was archived.';
COMMENT ON COLUMN projects.deleted_at IS
    'Timestamp when the project was soft-deleted (NULL if active).';

-- Create enhanced project members table
CREATE TABLE project_members
(
    -- Primary keys (composite)
    project_id         UUID         NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    account_id         UUID         NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,

    PRIMARY KEY (project_id, account_id),

    -- Role and permissions
    member_role        PROJECT_ROLE NOT NULL DEFAULT 'viewer',
    custom_permissions JSONB        NOT NULL DEFAULT '{}'::JSONB,

    CONSTRAINT project_members_custom_permissions_size_min CHECK (length(custom_permissions::TEXT) >= 2),
    CONSTRAINT project_members_custom_permissions_size_max CHECK (length(custom_permissions::TEXT) <= 2048),

    -- Member preferences and settings
    show_order         INTEGER      NOT NULL DEFAULT 0,
    is_favorite        BOOLEAN      NOT NULL DEFAULT FALSE,
    is_hidden          BOOLEAN      NOT NULL DEFAULT FALSE,

    CONSTRAINT project_members_show_order_min CHECK (show_order >= -1000),
    CONSTRAINT project_members_show_order_max CHECK (show_order <= 1000),

    -- Notification preferences
    notify_updates     BOOLEAN      NOT NULL DEFAULT TRUE,
    notify_comments    BOOLEAN      NOT NULL DEFAULT TRUE,
    notify_mentions    BOOLEAN      NOT NULL DEFAULT TRUE,

    -- Member status
    is_active          BOOLEAN      NOT NULL DEFAULT TRUE,
    last_accessed_at   TIMESTAMPTZ           DEFAULT NULL,

    -- Audit tracking
    created_by         UUID         NOT NULL REFERENCES accounts (id),
    updated_by         UUID         NOT NULL REFERENCES accounts (id),

    -- Lifecycle timestamps
    created_at         TIMESTAMPTZ  NOT NULL DEFAULT current_timestamp,
    updated_at         TIMESTAMPTZ  NOT NULL DEFAULT current_timestamp,

    -- Chronological integrity constraints
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

-- Primary keys
COMMENT ON COLUMN project_members.project_id IS
    'Reference to the project.';
COMMENT ON COLUMN project_members.account_id IS
    'Reference to the member account.';

-- Role and permissions
COMMENT ON COLUMN project_members.member_role IS
    'Member role defining base permissions level.';
COMMENT ON COLUMN project_members.custom_permissions IS
    'Custom permission overrides (JSON, 2B-2KB).';

-- Member preferences and settings
COMMENT ON COLUMN project_members.show_order IS
    'Custom sort order for member project list (-1000 to 1000).';
COMMENT ON COLUMN project_members.is_favorite IS
    'Mark project as favorite for quick access.';
COMMENT ON COLUMN project_members.is_hidden IS
    'Hide project from member project list.';

-- Notification preferences
COMMENT ON COLUMN project_members.notify_updates IS
    'Receive notifications for project updates.';
COMMENT ON COLUMN project_members.notify_comments IS
    'Receive notifications for new comments.';
COMMENT ON COLUMN project_members.notify_mentions IS
    'Receive notifications when mentioned.';

-- Member status
COMMENT ON COLUMN project_members.is_active IS
    'Member status (inactive members retain access but are hidden).';
COMMENT ON COLUMN project_members.last_accessed_at IS
    'Timestamp of member last project access.';

-- Audit tracking
COMMENT ON COLUMN project_members.created_by IS
    'Account that added this member.';
COMMENT ON COLUMN project_members.updated_by IS
    'Account that last modified this membership.';

-- Lifecycle timestamps
COMMENT ON COLUMN project_members.created_at IS
    'Timestamp when the membership was created.';
COMMENT ON COLUMN project_members.updated_at IS
    'Timestamp when the membership was last modified.';

-- Create enhanced project invites table
CREATE TABLE project_invites
(
    -- Unique invite identifier
    id             UUID PRIMARY KEY       DEFAULT gen_random_uuid(),

    -- References
    project_id     UUID          NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    invitee_email  TEXT          NOT NULL, -- Email of invited user (may not have account yet)
    invitee_id     UUID                   DEFAULT NULL REFERENCES accounts (id) ON DELETE SET NULL,

    CONSTRAINT project_invites_email_valid CHECK (is_valid_email(invitee_email)),

    -- Invitation details
    invited_role   PROJECT_ROLE  NOT NULL DEFAULT 'viewer',
    invite_message TEXT          NOT NULL DEFAULT '',
    invite_token   TEXT          NOT NULL DEFAULT generate_secure_token(32),

    CONSTRAINT project_invites_invite_message_length_max CHECK (length(invite_message) <= 1000),
    CONSTRAINT project_invites_invite_token_not_empty CHECK (trim(invite_token) <> ''),

    -- Invitation status and lifecycle
    invite_status  INVITE_STATUS NOT NULL DEFAULT 'pending',
    status_reason  TEXT                   DEFAULT NULL,

    CONSTRAINT project_invites_status_reason_length_max CHECK (status_reason IS NULL OR length(status_reason) <= 500),

    -- Expiration and limits
    expires_at     TIMESTAMPTZ   NOT NULL DEFAULT current_timestamp + INTERVAL '7 days',
    max_uses       INTEGER       NOT NULL DEFAULT 1,
    use_count      INTEGER       NOT NULL DEFAULT 0,

    CONSTRAINT project_invites_expires_in_future CHECK (expires_at > current_timestamp),
    CONSTRAINT project_invites_max_uses_min CHECK (max_uses >= 1),
    CONSTRAINT project_invites_max_uses_max CHECK (max_uses <= 100),
    CONSTRAINT project_invites_use_count_min CHECK (use_count >= 0),
    CONSTRAINT project_invites_use_count_max CHECK (use_count <= max_uses),

    -- Audit tracking
    created_by     UUID          NOT NULL REFERENCES accounts (id),
    updated_by     UUID          NOT NULL REFERENCES accounts (id),
    accepted_by    UUID                   DEFAULT NULL REFERENCES accounts (id),

    -- Lifecycle timestamps
    created_at     TIMESTAMPTZ   NOT NULL DEFAULT current_timestamp,
    updated_at     TIMESTAMPTZ   NOT NULL DEFAULT current_timestamp,
    deleted_at     TIMESTAMPTZ   NOT NULL DEFAULT current_timestamp,
    accepted_at    TIMESTAMPTZ            DEFAULT NULL,

    -- Chronological integrity constraints
    CONSTRAINT project_invites_updated_after_created CHECK (updated_at >= created_at),
    CONSTRAINT project_invites_deleted_after_updated CHECK (deleted_at IS NULL OR deleted_at >= updated_at),
    CONSTRAINT project_invites_accepted_after_created CHECK (accepted_at IS NULL OR accepted_at >= created_at),
    CONSTRAINT project_invites_expires_after_created CHECK (expires_at > created_at),

    -- Business logic constraints
    CONSTRAINT project_invites_accept_status_consistency CHECK (accepted_at IS NULL OR invite_status = 'accepted'),
    CONSTRAINT project_invites_acceptor_consistency CHECK ((accepted_by IS NULL) = (accepted_at IS NULL))
);

-- Set up automatic updated_at trigger
SELECT setup_updated_at('project_invites');

-- Create indexes for project invites
CREATE UNIQUE INDEX project_invites_token_unique_idx
    ON project_invites (invite_token)
    WHERE invite_status = 'pending';

CREATE INDEX project_invites_project_active_idx
    ON project_invites (project_id, invite_status, created_at DESC)
    WHERE invite_status IN ('pending', 'accepted');

CREATE INDEX project_invites_invitee_idx
    ON project_invites (invitee_email, invite_status)
    WHERE invite_status = 'pending';

CREATE INDEX project_invites_account_idx
    ON project_invites (invitee_id, invite_status, created_at DESC)
    WHERE invitee_id IS NOT NULL;

CREATE INDEX project_invites_expiration_idx
    ON project_invites (expires_at)
    WHERE invite_status = 'pending';

CREATE INDEX project_invites_creator_idx
    ON project_invites (created_by, created_at DESC);

-- Add comprehensive table and column comments
COMMENT ON TABLE project_invites IS
    'Enhanced project invitations with token-based system, expiration, and comprehensive tracking.';

-- Unique invite identifier
COMMENT ON COLUMN project_invites.id IS
    'Unique invite identifier (UUID).';

-- References
COMMENT ON COLUMN project_invites.project_id IS
    'Reference to the project being invited to.';
COMMENT ON COLUMN project_invites.invitee_email IS
    'Email address of the invited user.';
COMMENT ON COLUMN project_invites.invitee_id IS
    'Reference to invitee account (NULL if no account exists yet).';

-- Invitation details
COMMENT ON COLUMN project_invites.invited_role IS
    'Role the invitee will receive upon acceptance.';
COMMENT ON COLUMN project_invites.invite_message IS
    'Personal message from inviter (max 1000 chars).';
COMMENT ON COLUMN project_invites.invite_token IS
    'Secure token for invite acceptance (auto-generated).';

-- Invitation status and lifecycle
COMMENT ON COLUMN project_invites.invite_status IS
    'Current status of the invitation.';
COMMENT ON COLUMN project_invites.status_reason IS
    'Optional reason for status changes (max 500 chars).';

-- Expiration and limits
COMMENT ON COLUMN project_invites.expires_at IS
    'Timestamp when invitation expires.';
COMMENT ON COLUMN project_invites.max_uses IS
    'Maximum number of times invite can be used (1-100).';
COMMENT ON COLUMN project_invites.use_count IS
    'Number of times invite has been used.';

-- Audit tracking
COMMENT ON COLUMN project_invites.created_by IS
    'Account that created the invitation.';
COMMENT ON COLUMN project_invites.updated_by IS
    'Account that last modified the invitation.';
COMMENT ON COLUMN project_invites.accepted_by IS
    'Account that accepted the invitation (may differ from invitee_id).';

-- Lifecycle timestamps
COMMENT ON COLUMN project_invites.created_at IS
    'Timestamp when the invitation was created.';
COMMENT ON COLUMN project_invites.updated_at IS
    'Timestamp when the invitation was last modified.';
COMMENT ON COLUMN project_invites.accepted_at IS
    'Timestamp when the invitation was accepted.';

-- Create project activity log table
CREATE TABLE project_activity_log
(
    -- Primary identifier
    id            BIGSERIAL PRIMARY KEY,

    -- References
    project_id    UUID        NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    actor_id      UUID                 DEFAULT NULL REFERENCES accounts (id) ON DELETE SET NULL,

    -- Activity details
    activity_type TEXT        NOT NULL,
    activity_data JSONB       NOT NULL DEFAULT '{}'::JSONB,
    entity_type   TEXT                 DEFAULT NULL, -- 'document', 'member', 'invite', etc.
    entity_id     UUID                 DEFAULT NULL,

    CONSTRAINT project_activity_log_activity_type_not_empty CHECK (trim(activity_type) <> ''),
    CONSTRAINT project_activity_log_activity_type_length_max CHECK (length(activity_type) <= 100),
    CONSTRAINT project_activity_log_activity_data_size_min CHECK (length(activity_data::TEXT) >= 2),
    CONSTRAINT project_activity_log_activity_data_size_max CHECK (length(activity_data::TEXT) <= 4096),
    CONSTRAINT project_activity_log_entity_type_length_max CHECK (entity_type IS NULL OR length(entity_type) <= 50),

    -- Context
    ip_address    INET                 DEFAULT NULL,
    user_agent    TEXT                 DEFAULT NULL,

    -- Timestamp
    created_at    TIMESTAMPTZ NOT NULL DEFAULT current_timestamp
);

-- Create indexes for activity log
CREATE INDEX project_activity_log_project_idx
    ON project_activity_log (project_id, created_at DESC);

CREATE INDEX project_activity_log_actor_idx
    ON project_activity_log (actor_id, created_at DESC)
    WHERE actor_id IS NOT NULL;

CREATE INDEX project_activity_log_type_idx
    ON project_activity_log (activity_type, project_id, created_at DESC);

CREATE INDEX project_activity_log_entity_idx
    ON project_activity_log (entity_type, entity_id, created_at DESC)
    WHERE entity_type IS NOT NULL AND entity_id IS NOT NULL;

-- Add comprehensive table and column comments
COMMENT ON TABLE project_activity_log IS
    'Comprehensive activity log for project events and member actions.';

-- Primary identifier
COMMENT ON COLUMN project_activity_log.id IS
    'Unique activity log entry identifier (auto-incrementing).';

-- References
COMMENT ON COLUMN project_activity_log.project_id IS
    'Reference to the project where activity occurred.';
COMMENT ON COLUMN project_activity_log.actor_id IS
    'Reference to the account that performed the activity (NULL for system actions).';

-- Activity details
COMMENT ON COLUMN project_activity_log.activity_type IS
    'Type of activity performed (max 100 chars).';
COMMENT ON COLUMN project_activity_log.activity_data IS
    'Additional activity context and data (JSON, 2B-4KB).';
COMMENT ON COLUMN project_activity_log.entity_type IS
    'Type of entity affected by the activity (max 50 chars).';
COMMENT ON COLUMN project_activity_log.entity_id IS
    'ID of the specific entity affected by the activity.';

-- Context
COMMENT ON COLUMN project_activity_log.ip_address IS
    'IP address from which the activity originated.';
COMMENT ON COLUMN project_activity_log.user_agent IS
    'User agent of the client that performed the activity.';

-- Timestamp
COMMENT ON COLUMN project_activity_log.created_at IS
    'Timestamp when the activity occurred.';

-- Create useful views for common queries
CREATE VIEW project_member_summary AS
SELECT p.id                                                  AS project_id,
       p.display_name,
       p.status                                              AS project_status,
       count(pm.account_id)                                  AS total_members,
       count(CASE WHEN pm.member_role = 'owner' THEN 1 END)  AS owners,
       count(CASE WHEN pm.member_role = 'admin' THEN 1 END)  AS admins,
       count(CASE WHEN pm.member_role = 'editor' THEN 1 END) AS editors,
       count(CASE WHEN pm.member_role = 'viewer' THEN 1 END) AS viewers,
       count(CASE WHEN pm.is_active = FALSE THEN 1 END)      AS inactive_members,
       max(pm.last_accessed_at)                              AS last_member_access
FROM projects p
         LEFT JOIN project_members pm ON p.id = pm.project_id
WHERE p.deleted_at IS NULL
GROUP BY p.id, p.display_name, p.status;

COMMENT ON VIEW project_member_summary IS
    'Summary view of project membership statistics and activity.';

CREATE VIEW pending_project_invites AS
SELECT pi.id,
       pi.project_id,
       p.display_name                      AS project_name,
       pi.invitee_email,
       pi.invited_role,
       pi.invite_message,
       pi.created_by,
       creator.display_name                AS inviter_name,
       pi.created_at,
       pi.expires_at,
       (pi.expires_at < current_timestamp) AS is_expired
FROM project_invites pi
         JOIN projects p ON pi.project_id = p.id
         JOIN accounts creator ON pi.created_by = creator.id
WHERE pi.invite_status = 'pending'
  AND p.deleted_at IS NULL;

COMMENT ON VIEW pending_project_invites IS
    'Active project invitations with project and inviter details.';

-- Create functions for common project operations

-- Function to check if user has specific permission on project
CREATE OR REPLACE FUNCTION check_project_permission(
    _project_id UUID,
    _account_id UUID,
    _required_permission TEXT
) RETURNS BOOLEAN AS
$$
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

    -- If not a member, check if it's a public project with appropriate visibility
    IF _member_role IS NULL THEN
        SELECT (visibility = 'public' AND _required_permission = 'view')
        INTO _has_permission
        FROM projects
        WHERE id = _project_id
          AND status = 'active'
          AND deleted_at IS NULL;

        RETURN coalesce(_has_permission, FALSE);
    END IF;

    -- Check role-based permissions
    _has_permission := CASE _required_permission
                           WHEN 'view' THEN TRUE -- All members can view
                           WHEN 'comment' THEN _member_role != 'viewer'
                           WHEN 'edit' THEN _member_role IN ('owner', 'admin', 'editor')
                           WHEN 'manage_members' THEN _member_role IN ('owner', 'admin')
                           WHEN 'admin' THEN _member_role IN ('owner', 'admin')
                           WHEN 'delete' THEN _member_role = 'owner'
                           ELSE FALSE
        END;

    -- Check custom permission overrides (if any)
    IF _custom_perms IS NOT NULL AND _custom_perms ? _required_permission THEN
        _has_permission := (_custom_perms ->> _required_permission)::BOOLEAN;
    END IF;

    RETURN _has_permission;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION check_project_permission(UUID, UUID, TEXT) IS
    'Checks if a user has specific permission on a project, considering role hierarchy and custom permissions.';

-- Function to cleanup expired invites
CREATE OR REPLACE FUNCTION cleanup_expired_invites() RETURNS INTEGER AS
$$
DECLARE
    _expired_count INTEGER;
BEGIN
    WITH expired_invites AS (
        UPDATE project_invites
            SET invite_status = 'expired',
                updated_by = created_by, -- Use original creator as updater
                updated_at = current_timestamp
            WHERE invite_status = 'pending'
                AND expires_at < current_timestamp
            RETURNING 1)
    SELECT count(*)
    INTO _expired_count
    FROM expired_invites;

    RETURN _expired_count;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION cleanup_expired_invites() IS
    'Marks expired project invitations as expired. Returns count of expired invitations.';

-- Create enum for integration status
CREATE TYPE INTEGRATION_STATUS AS ENUM (
    'pending',
    'executing',
    'failure'
    );

COMMENT ON TYPE INTEGRATION_STATUS IS
    'Defines the operational status of project integrations.';

-- Create project_integrations table
CREATE TABLE project_integrations
(
    -- Primary identifier
    id               UUID PRIMARY KEY            DEFAULT gen_random_uuid(),

    -- Reference
    project_id       UUID               NOT NULL REFERENCES projects (id) ON DELETE CASCADE,

    -- Integration details
    integration_name TEXT               NOT NULL,
    description      TEXT               NOT NULL DEFAULT '',

    CONSTRAINT project_integrations_name_length_min CHECK (char_length(trim(integration_name)) >= 2),
    CONSTRAINT project_integrations_name_length_max CHECK (char_length(trim(integration_name)) <= 100),
    CONSTRAINT project_integrations_description_length_max CHECK (char_length(description) <= 1000),

    -- Status and configuration
    status           INTEGRATION_STATUS NOT NULL DEFAULT 'pending',
    is_enabled       BOOLEAN            NOT NULL DEFAULT TRUE,

    -- Authentication data and metadata
    auth_data        JSONB              NOT NULL DEFAULT '{}'::JSONB,
    metadata         JSONB              NOT NULL DEFAULT '{}'::JSONB,

    CONSTRAINT project_integrations_auth_data_size_min CHECK (length(auth_data::TEXT) >= 2),
    CONSTRAINT project_integrations_auth_data_size_max CHECK (length(auth_data::TEXT) <= 16384),
    CONSTRAINT project_integrations_metadata_size_min CHECK (length(metadata::TEXT) >= 2),
    CONSTRAINT project_integrations_metadata_size_max CHECK (length(metadata::TEXT) <= 8192),

    -- Audit fields
    created_by       UUID               NOT NULL REFERENCES accounts (id),
    updated_by       UUID               NOT NULL REFERENCES accounts (id),
    created_at       TIMESTAMPTZ        NOT NULL DEFAULT current_timestamp,
    updated_at       TIMESTAMPTZ        NOT NULL DEFAULT current_timestamp,
    deleted_at       TIMESTAMPTZ,

    -- Chronological integrity constraints
    CONSTRAINT project_integrations_updated_after_created CHECK (updated_at >= created_at),
    CONSTRAINT project_integrations_deleted_after_updated CHECK (deleted_at IS NULL OR deleted_at >= updated_at)
);

-- Set up automatic updated_at trigger
SELECT setup_updated_at('project_integrations');

-- Create indexes for project_integrations
CREATE INDEX project_integrations_project_id_idx
    ON project_integrations (project_id, status)
    WHERE deleted_at IS NULL;

CREATE INDEX project_integrations_status_idx
    ON project_integrations (status, is_enabled)
    WHERE deleted_at IS NULL AND is_enabled = TRUE;

CREATE UNIQUE INDEX project_integrations_unique_active_idx
    ON project_integrations (project_id, lower(integration_name))
    WHERE deleted_at IS NULL;

-- Add table and column comments
COMMENT ON TABLE project_integrations IS
    'Dynamic third-party integrations connected to projects with flexible authentication and metadata.';

COMMENT ON COLUMN project_integrations.id IS
    'Unique integration identifier (UUID).';
COMMENT ON COLUMN project_integrations.project_id IS
    'Reference to the project this integration belongs to.';
COMMENT ON COLUMN project_integrations.integration_name IS
    'Human-readable name for this integration (2-100 characters).';
COMMENT ON COLUMN project_integrations.description IS
    'Description of what this integration does (max 1000 characters).';
COMMENT ON COLUMN project_integrations.status IS
    'Current operational status of the integration.';
COMMENT ON COLUMN project_integrations.is_enabled IS
    'Whether the integration is enabled and should be active.';
COMMENT ON COLUMN project_integrations.auth_data IS
    'Authentication data including method, credentials, and tokens (JSON, 2B-16KB).';
COMMENT ON COLUMN project_integrations.metadata IS
    'Additional integration metadata and configuration (JSON, 2B-8KB).';
COMMENT ON COLUMN project_integrations.created_by IS
    'Account that created this integration.';
COMMENT ON COLUMN project_integrations.updated_by IS
    'Account that last updated this integration.';
COMMENT ON COLUMN project_integrations.created_at IS
    'Timestamp when the integration was created.';
COMMENT ON COLUMN project_integrations.updated_at IS
    'Timestamp when the integration was last updated.';
COMMENT ON COLUMN project_integrations.deleted_at IS
    'Timestamp when the integration was soft-deleted (NULL if active).';
