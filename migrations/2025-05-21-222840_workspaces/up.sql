-- Workspaces: the multi-tenant unit that owns documents and connections. This
-- migration defines the workspace itself, its membership with per-member roles
-- and notification preferences, and the invitation flow.

-- Workspaces table: a tenant that owns content and members.
CREATE TABLE workspaces (
    -- Primary identifier
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Identity and branding
    display_name     TEXT             NOT NULL,
    CONSTRAINT workspaces_display_name_length CHECK (length(trim(display_name)) BETWEEN 2 AND 32),

    -- Human-readable URL identity, unique among live workspaces (enforced by a
    -- partial index below so a slug frees up after soft deletion). Mirrors the
    -- WorkspaceSlug newtype: lowercase alphanumeric with single internal dashes,
    -- 3-32 characters.
    slug             TEXT             NOT NULL,
    CONSTRAINT workspaces_slug_length CHECK (length(slug) BETWEEN 3 AND 32),
    CONSTRAINT workspaces_slug_format CHECK (slug ~ '^[a-z0-9]+(-[a-z0-9]+)*$'),

    description      TEXT             DEFAULT NULL,
    CONSTRAINT workspaces_description_length_max CHECK (length(description) <= 500),

    avatar_url       TEXT             DEFAULT NULL,

    -- Extended metadata
    metadata         JSONB              NOT NULL DEFAULT '{}',
    CONSTRAINT workspaces_metadata_size CHECK (length(metadata::TEXT) BETWEEN 2 AND 8192),

    settings         JSONB              NOT NULL DEFAULT '{}',
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

-- Auto-maintain updated_at (and guard soft-delete) on writes.
SELECT setup_updated_at('workspaces');

-- One live workspace per slug (frees the slug after soft deletion).
CREATE UNIQUE INDEX workspaces_slug_unique_idx
    ON workspaces (slug)
    WHERE deleted_at IS NULL;

-- One live workspace per display name and owner.
CREATE UNIQUE INDEX workspaces_display_name_owner_unique_idx
    ON workspaces (lower(display_name), created_by)
    WHERE deleted_at IS NULL;

-- Fast existence/lookup of a live workspace by id.
CREATE INDEX workspaces_active_lookup_idx
    ON workspaces (id)
    WHERE deleted_at IS NULL;

-- An owner's live workspaces, most recent first.
CREATE INDEX workspaces_owner_lookup_idx
    ON workspaces (created_by, created_at DESC)
    WHERE deleted_at IS NULL;

-- Containment queries over live-workspace metadata.
CREATE INDEX workspaces_metadata_lookup_idx
    ON workspaces USING gin (metadata)
    WHERE deleted_at IS NULL;

-- Fuzzy display-name search over live workspaces.
CREATE INDEX workspaces_display_name_trgm_idx
    ON workspaces USING gin (display_name gin_trgm_ops)
    WHERE deleted_at IS NULL;

COMMENT ON TABLE workspaces IS 'Multi-tenant workspaces that own documents, members, and connections.';
COMMENT ON COLUMN workspaces.id IS 'Unique workspace identifier';
COMMENT ON COLUMN workspaces.display_name IS 'Human-readable workspace name (2-32 characters)';
COMMENT ON COLUMN workspaces.slug IS 'Unique URL slug among live workspaces (3-32 chars, lowercase)';
COMMENT ON COLUMN workspaces.description IS 'Optional workspace description (up to 500 characters)';
COMMENT ON COLUMN workspaces.avatar_url IS 'URL to workspace avatar/logo image';
COMMENT ON COLUMN workspaces.metadata IS 'Extended workspace metadata (JSON, 2B-8KB)';
COMMENT ON COLUMN workspaces.settings IS 'Typed workspace settings (JSON, 2B-8KB): approval requirement, data-retention rules';
COMMENT ON COLUMN workspaces.created_by IS 'Account that created this workspace (becomes first owner)';
COMMENT ON COLUMN workspaces.created_at IS 'Workspace creation timestamp';
COMMENT ON COLUMN workspaces.updated_at IS 'Timestamp of the most recent modification (auto-updated)';
COMMENT ON COLUMN workspaces.deleted_at IS 'Soft-deletion timestamp; NULL means live';

-- Access role of a workspace member: hierarchical permission levels.
CREATE TYPE WORKSPACE_ROLE AS ENUM (
    'owner',        -- Full workspace ownership and management
    'admin',        -- Can manage members, connections, and settings
    'editor',       -- Can edit content and download original files
    'reviewer'      -- Can review redacted output and audits, not originals
);

COMMENT ON TYPE WORKSPACE_ROLE IS 'Access role of a workspace member: owner, admin, editor, or reviewer.';

-- Workspace members table: an account's membership in a workspace.
CREATE TABLE workspace_members (
    -- Primary key (composite)
    workspace_id       UUID           NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    account_id         UUID           NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    PRIMARY KEY (workspace_id, account_id),

    -- Role
    member_role        WORKSPACE_ROLE NOT NULL DEFAULT 'reviewer',

    -- Notification preferences
    notify_via_email          BOOLEAN              NOT NULL DEFAULT FALSE,
    -- In-app defaults to every event so members are notified out of the box;
    -- a member narrows the set by replacing it. Keep this list in sync with the
    -- NOTIFICATION_EVENT enum when it changes. Email stays opt-in (empty).
    notification_events_app   NOTIFICATION_EVENT[] NOT NULL DEFAULT ARRAY[
        'member.invited', 'member.joined',
        'connection.sync.completed', 'connection.sync.failed',
        'pipeline.detection.completed', 'pipeline.redaction.created', 'pipeline.detection.failed'
    ]::NOTIFICATION_EVENT[],
    notification_events_email NOTIFICATION_EVENT[] NOT NULL DEFAULT '{}',

    -- Audit tracking
    created_by         UUID           NOT NULL REFERENCES accounts (id),
    updated_by         UUID           NOT NULL REFERENCES accounts (id),

    -- Lifecycle timestamps
    created_at         TIMESTAMPTZ    NOT NULL DEFAULT current_timestamp,
    updated_at         TIMESTAMPTZ    NOT NULL DEFAULT current_timestamp,
    CONSTRAINT workspace_members_updated_after_created CHECK (updated_at >= created_at)
);

-- Auto-maintain updated_at on writes (no soft-delete column here).
SELECT setup_updated_at_no_soft_delete('workspace_members');

-- An account's workspaces, most recent first.
CREATE INDEX workspace_members_account_workspaces_idx
    ON workspace_members (account_id, created_at DESC);

-- A workspace's members grouped by role.
CREATE INDEX workspace_members_workspace_role_idx
    ON workspace_members (workspace_id, member_role);

-- Members of a given role across a workspace.
CREATE INDEX workspace_members_role_lookup_idx
    ON workspace_members (member_role, workspace_id);

COMMENT ON TABLE workspace_members IS 'Workspace membership with roles and notification preferences.';
COMMENT ON COLUMN workspace_members.workspace_id IS 'Workspace this membership belongs to';
COMMENT ON COLUMN workspace_members.account_id IS 'Account that is a member';
COMMENT ON COLUMN workspace_members.member_role IS 'Member role defining base permission level';
COMMENT ON COLUMN workspace_members.notify_via_email IS 'Whether to send email notifications';
COMMENT ON COLUMN workspace_members.notification_events_app IS 'Notification events to receive in-app';
COMMENT ON COLUMN workspace_members.notification_events_email IS 'Notification events to receive via email';
COMMENT ON COLUMN workspace_members.created_by IS 'Account that added this member';
COMMENT ON COLUMN workspace_members.updated_by IS 'Account that last modified this membership';
COMMENT ON COLUMN workspace_members.created_at IS 'Membership creation timestamp';
COMMENT ON COLUMN workspace_members.updated_at IS 'Timestamp of the most recent modification';

-- Lifecycle status of a workspace invitation.
CREATE TYPE INVITE_STATUS AS ENUM (
    'pending',      -- Invitation sent, awaiting response
    'accepted',     -- Invitation accepted, member added
    'declined',     -- Invitation declined by invitee
    'canceled',     -- Invitation canceled by inviter
    'expired',      -- Invitation expired due to timeout
    'revoked'       -- Invitation revoked by admin
);

COMMENT ON TYPE INVITE_STATUS IS 'Lifecycle status of a workspace invitation.';

-- Workspace invites table: an invitation to join a workspace.
CREATE TABLE workspace_invites (
    -- Primary identifier
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References
    workspace_id   UUID            NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,

    -- Composite key target for workspace-scoped access and foreign keys.
    CONSTRAINT workspace_invites_workspace_id_id_key UNIQUE (workspace_id, id),

    -- Invitation details
    invitee_email  TEXT            DEFAULT NULL,
    CONSTRAINT workspace_invites_invitee_email_format CHECK (invitee_email IS NULL OR is_valid_email(invitee_email)),

    invited_role   WORKSPACE_ROLE  NOT NULL DEFAULT 'reviewer',

    invite_token   TEXT            NOT NULL DEFAULT generate_secure_token(32),
    CONSTRAINT workspace_invites_invite_token_not_empty CHECK (trim(invite_token) <> ''),

    -- Status and expiration
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

-- Auto-maintain updated_at on writes (no soft-delete column here).
SELECT setup_updated_at_no_soft_delete('workspace_invites');

-- Validate a pending invite by its token.
CREATE INDEX workspace_invites_token_lookup_idx
    ON workspace_invites (invite_token)
    WHERE invite_status = 'pending';

-- Find pending invites due for expiry, ordered by deadline.
CREATE INDEX workspace_invites_expiry_cleanup_idx
    ON workspace_invites (expires_at)
    WHERE invite_status = 'pending';

-- An invitee's invitations by status, most recent first.
CREATE INDEX workspace_invites_invitee_lookup_idx
    ON workspace_invites (invitee_email, invite_status, created_at DESC)
    WHERE invitee_email IS NOT NULL;

COMMENT ON TABLE workspace_invites IS 'Invitations to join a workspace, with status tracking and secure tokens.';
COMMENT ON COLUMN workspace_invites.id IS 'Unique invite identifier';
COMMENT ON COLUMN workspace_invites.workspace_id IS 'Workspace the invite grants access to';
COMMENT ON COLUMN workspace_invites.invitee_email IS 'Email address of invitee (NULL for open invite links)';
COMMENT ON COLUMN workspace_invites.invited_role IS 'Role assigned upon acceptance';
COMMENT ON COLUMN workspace_invites.invite_token IS 'Secure token for invite validation';
COMMENT ON COLUMN workspace_invites.invite_status IS 'Current status of the invitation';
COMMENT ON COLUMN workspace_invites.expires_at IS 'Invitation expiration timestamp';
COMMENT ON COLUMN workspace_invites.responded_at IS 'Timestamp when the invitee responded';
COMMENT ON COLUMN workspace_invites.created_by IS 'Account that sent the invitation';
COMMENT ON COLUMN workspace_invites.updated_by IS 'Account that last modified the invitation';
COMMENT ON COLUMN workspace_invites.created_at IS 'Invitation creation timestamp';
COMMENT ON COLUMN workspace_invites.updated_at IS 'Timestamp of the most recent modification';
