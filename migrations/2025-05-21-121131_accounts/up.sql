-- This migration creates tables for accounts, sessions and tokens

-- Create api_token_type enum
CREATE TYPE API_TOKEN_TYPE AS ENUM (
    'web',      -- Web browser sessions
    'mobile',   -- Mobile app sessions
    'api',      -- API client sessions
    'desktop'   -- Desktop application sessions
);

COMMENT ON TYPE API_TOKEN_TYPE IS
    'Enumeration of supported API token types for authentication and tracking purposes.';

-- Create accounts table with security and validation
CREATE TABLE accounts (
    -- Primary identifiers
    id                    UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Account status and permissions
    is_admin              BOOLEAN     NOT NULL DEFAULT FALSE,
    is_verified           BOOLEAN     NOT NULL DEFAULT FALSE,
    is_suspended          BOOLEAN     NOT NULL DEFAULT FALSE,

    -- Core account information
    display_name          TEXT        NOT NULL,
    email_address         TEXT        NOT NULL,
    password_hash         TEXT        NOT NULL,

    CONSTRAINT accounts_display_name_length CHECK (length(trim(display_name)) BETWEEN 2 AND 100),
    CONSTRAINT accounts_display_name_not_empty CHECK (trim(display_name) <> ''),
    CONSTRAINT accounts_email_format CHECK (is_valid_email(email_address)),
    CONSTRAINT accounts_email_length_max CHECK (length(email_address) <= 254),
    CONSTRAINT accounts_password_hash_not_empty CHECK (password_hash <> ''),
    CONSTRAINT accounts_password_hash_length_min CHECK (length(password_hash) >= 60),

    -- Optional profile information
    company_name          TEXT        DEFAULT NULL,
    avatar_url            TEXT        DEFAULT NULL,

    CONSTRAINT accounts_company_name_length_max CHECK (company_name IS NULL OR length(company_name) <= 255),

    -- Preferences and settings
    timezone              TEXT        NOT NULL DEFAULT 'UTC',
    locale                TEXT        NOT NULL DEFAULT 'en-US',

    CONSTRAINT accounts_timezone_format CHECK (timezone ~ '^[A-Za-z_/]+$'),
    CONSTRAINT accounts_locale_format CHECK (locale ~ '^[a-z]{2}-[A-Z]{2}$'),

    -- Security tracking
    password_changed_at   TIMESTAMPTZ DEFAULT NULL,

    -- Lifecycle timestamps
    created_at            TIMESTAMPTZ NOT NULL DEFAULT current_timestamp,
    updated_at            TIMESTAMPTZ NOT NULL DEFAULT current_timestamp,
    deleted_at            TIMESTAMPTZ DEFAULT NULL,

    CONSTRAINT accounts_updated_after_created CHECK (updated_at >= created_at),
    CONSTRAINT accounts_deleted_after_created CHECK (deleted_at IS NULL OR deleted_at >= created_at),
    CONSTRAINT accounts_deleted_after_updated CHECK (deleted_at IS NULL OR deleted_at >= updated_at),
    CONSTRAINT accounts_password_changed_after_created CHECK (password_changed_at IS NULL OR password_changed_at >= created_at),

    -- Business logic constraints
    CONSTRAINT accounts_suspended_not_admin CHECK (NOT (is_suspended AND is_admin))
);

-- Set up automatic updated_at trigger
SELECT setup_updated_at('accounts');

-- Create indexes for accounts
CREATE UNIQUE INDEX accounts_email_address_unique_idx
    ON accounts (lower(email_address))
    WHERE deleted_at IS NULL;

CREATE INDEX accounts_admin_users_idx
    ON accounts (id, display_name)
    WHERE is_admin = TRUE AND deleted_at IS NULL;



-- Add table and column comments
COMMENT ON TABLE accounts IS
    'User accounts with enhanced security features, preferences, and audit trails.';

COMMENT ON COLUMN accounts.id IS 'Unique account identifier (UUID)';
COMMENT ON COLUMN accounts.is_admin IS 'Administrative privileges across the entire system';
COMMENT ON COLUMN accounts.is_verified IS 'Account identity verification status (email confirmation, etc.)';
COMMENT ON COLUMN accounts.is_suspended IS 'Temporarily disables account access while preserving data';
COMMENT ON COLUMN accounts.display_name IS 'Human-readable name for UI and communications (2-100 characters)';
COMMENT ON COLUMN accounts.email_address IS 'Primary email for authentication and communications (validated format)';
COMMENT ON COLUMN accounts.password_hash IS 'Securely hashed password (bcrypt recommended, minimum 60 characters)';
COMMENT ON COLUMN accounts.company_name IS 'Optional company affiliation for business accounts';
COMMENT ON COLUMN accounts.avatar_url IS 'URL to user profile image or avatar';
COMMENT ON COLUMN accounts.timezone IS 'User timezone for date/time display preferences';
COMMENT ON COLUMN accounts.locale IS 'User locale for language and regional formatting';
COMMENT ON COLUMN accounts.password_changed_at IS 'Timestamp of last password change for security tracking';
COMMENT ON COLUMN accounts.created_at IS 'Timestamp when the account was created';
COMMENT ON COLUMN accounts.updated_at IS 'Timestamp when the account was last modified (auto-updated by trigger)';
COMMENT ON COLUMN accounts.deleted_at IS 'Timestamp when the account was soft-deleted (NULL if active)';

-- Create account API tokens table
CREATE TABLE account_api_tokens (
    -- Primary identifier
    id                    UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Account reference
    account_id            UUID        NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,

    -- Token metadata
    name                  TEXT        NOT NULL,
    session_type          API_TOKEN_TYPE NOT NULL DEFAULT 'web',

    CONSTRAINT account_api_tokens_name_not_empty CHECK (trim(name) <> ''),
    CONSTRAINT account_api_tokens_name_length CHECK (length(name) <= 100),

    -- Security context
    ip_address            INET        DEFAULT NULL,
    user_agent            TEXT        DEFAULT NULL,
    is_remembered         BOOLEAN     NOT NULL DEFAULT FALSE,

    -- Lifecycle timestamps
    issued_at             TIMESTAMPTZ NOT NULL DEFAULT current_timestamp,
    expired_at            TIMESTAMPTZ DEFAULT NULL,
    last_used_at          TIMESTAMPTZ DEFAULT NULL,
    deleted_at            TIMESTAMPTZ DEFAULT NULL,

    CONSTRAINT account_api_tokens_expired_after_issued CHECK (expired_at IS NULL OR expired_at > issued_at),
    CONSTRAINT account_api_tokens_deleted_after_issued CHECK (deleted_at IS NULL OR deleted_at >= issued_at),
    CONSTRAINT account_api_tokens_last_used_after_issued CHECK (last_used_at IS NULL OR last_used_at >= issued_at)
);

-- Create indexes for API token management
CREATE INDEX account_api_tokens_account_active_idx
    ON account_api_tokens (account_id, expired_at)
    WHERE deleted_at IS NULL;

CREATE INDEX account_api_tokens_cleanup_idx
    ON account_api_tokens (expired_at)
    WHERE deleted_at IS NULL;

-- Add table and column comments
COMMENT ON TABLE account_api_tokens IS
    'API tokens for user authentication sessions.';

COMMENT ON COLUMN account_api_tokens.id IS 'Unique token identifier (UUID primary key)';
COMMENT ON COLUMN account_api_tokens.account_id IS 'Reference to the account this token belongs to';
COMMENT ON COLUMN account_api_tokens.name IS 'Human-readable name for the token (max 100 characters)';
COMMENT ON COLUMN account_api_tokens.session_type IS 'Type of client that created the session (web, mobile, api, desktop)';
COMMENT ON COLUMN account_api_tokens.ip_address IS 'IP address where the session was created';
COMMENT ON COLUMN account_api_tokens.user_agent IS 'User agent of the client that created the session';
COMMENT ON COLUMN account_api_tokens.is_remembered IS 'Whether the session uses extended expiration (remember me)';
COMMENT ON COLUMN account_api_tokens.issued_at IS 'Timestamp when the token was created';
COMMENT ON COLUMN account_api_tokens.expired_at IS 'Timestamp when the token expires';
COMMENT ON COLUMN account_api_tokens.last_used_at IS 'Timestamp of most recent token usage';
COMMENT ON COLUMN account_api_tokens.deleted_at IS 'Timestamp when the token was revoked (NULL if active)';

-- Create comprehensive action token type enum
CREATE TYPE ACTION_TOKEN_TYPE AS ENUM (
    'activate_account',     -- Email verification for new accounts
    'deactivate_account',   -- Account suspension/deactivation
    'update_email',         -- Email address change verification
    'reset_password',       -- Password reset via email
    'change_password',      -- Password change verification
    'enable_2fa',           -- Two-factor authentication setup
    'disable_2fa',          -- Two-factor authentication removal
    'login_verification',   -- Additional login verification
    'api_access',           -- API access tokens
    'import_data',          -- Data import authorization
    'export_data'           -- Data export authorization
);

COMMENT ON TYPE ACTION_TOKEN_TYPE IS
    'Comprehensive enumeration of all token-based action operations and verifications.';

-- Create account action tokens table
CREATE TABLE account_action_tokens (
    -- Token identifiers
    action_token          UUID        NOT NULL DEFAULT gen_random_uuid(),
    account_id            UUID        NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,

    CONSTRAINT account_action_tokens_pkey PRIMARY KEY (account_id, action_token),

    -- Token purpose and data
    action_type           ACTION_TOKEN_TYPE NOT NULL,
    action_data           JSONB       NOT NULL DEFAULT '{}',

    CONSTRAINT account_action_tokens_action_data_size CHECK (length(action_data::TEXT) BETWEEN 2 AND 4096),

    -- Security context
    ip_address            INET        DEFAULT NULL,
    user_agent            TEXT        DEFAULT NULL,
    device_id             TEXT        DEFAULT NULL,

    -- Token lifecycle
    issued_at             TIMESTAMPTZ NOT NULL DEFAULT current_timestamp,
    expired_at            TIMESTAMPTZ NOT NULL DEFAULT current_timestamp + INTERVAL '24 hours',
    used_at               TIMESTAMPTZ DEFAULT NULL,

    CONSTRAINT account_action_tokens_expired_after_issued CHECK (expired_at > issued_at),
    CONSTRAINT account_action_tokens_used_after_issued CHECK (used_at IS NULL OR used_at >= issued_at),
    CONSTRAINT account_action_tokens_used_before_expired CHECK (used_at IS NULL OR used_at <= expired_at)
);

-- Create indexes for action token management
CREATE INDEX account_action_tokens_action_type_idx
    ON account_action_tokens (action_type, account_id, expired_at)
    WHERE used_at IS NULL;

CREATE INDEX account_action_tokens_cleanup_idx
    ON account_action_tokens (expired_at)
    WHERE used_at IS NULL;

CREATE INDEX account_action_tokens_device_tracking_idx
    ON account_action_tokens (account_id, device_id, issued_at DESC)
    WHERE device_id IS NOT NULL;

-- Add table and column comments
COMMENT ON TABLE account_action_tokens IS
    'Secure, time-limited tokens for various account operations with comprehensive tracking and rate limiting.';

COMMENT ON COLUMN account_action_tokens.action_token IS 'Unique identifier for the token (UUID)';
COMMENT ON COLUMN account_action_tokens.account_id IS 'Reference to the account this token belongs to';
COMMENT ON COLUMN account_action_tokens.action_type IS 'Type of action this token authorizes (from ACTION_TOKEN_TYPE enum)';
COMMENT ON COLUMN account_action_tokens.action_data IS 'Additional context data for the token action (JSON, 2B-4KB)';
COMMENT ON COLUMN account_action_tokens.ip_address IS 'IP address where the token was generated';
COMMENT ON COLUMN account_action_tokens.user_agent IS 'User agent of the client that generated the token';
COMMENT ON COLUMN account_action_tokens.device_id IS 'Optional device identifier for additional security tracking';
COMMENT ON COLUMN account_action_tokens.issued_at IS 'Timestamp when the token was created';
COMMENT ON COLUMN account_action_tokens.expired_at IS 'Timestamp after which the token becomes invalid';
COMMENT ON COLUMN account_action_tokens.used_at IS 'Timestamp when the token was successfully used (NULL if unused)';

-- Create notification event enum
CREATE TYPE NOTIFICATION_EVENT AS ENUM (
    -- Comment events
    'comment:mention',        -- User was mentioned in a comment
    'comment:reply',          -- Someone replied to user's comment

    -- Document events
    'document:uploaded',      -- Document was uploaded
    'document:downloaded',    -- Document was downloaded
    'document:verified',      -- Document verification completed

    -- Member events
    'member:invited',         -- User was invited to a workspace
    'member:joined',          -- A new member joined a workspace

    -- Integration events
    'integration:synced',     -- Integration sync completed
    'integration:desynced',   -- Integration sync failed or disconnected

    -- System events
    'system:announcement',    -- System-wide announcement
    'system:report'           -- System report generated
);

COMMENT ON TYPE NOTIFICATION_EVENT IS
    'Types of notification events that can be sent to users.';

-- Create account notifications table
CREATE TABLE account_notifications (
    -- Primary identifiers
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References
    account_id      UUID             NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,

    -- Notification details
    notify_type     NOTIFICATION_EVENT NOT NULL,
    title           TEXT             NOT NULL,
    message         TEXT             NOT NULL,

    CONSTRAINT account_notifications_title_length CHECK (length(trim(title)) BETWEEN 1 AND 200),
    CONSTRAINT account_notifications_message_length CHECK (length(trim(message)) BETWEEN 1 AND 1000),

    -- Status tracking
    is_read         BOOLEAN          NOT NULL DEFAULT FALSE,
    read_at         TIMESTAMPTZ      DEFAULT NULL,

    -- Optional related entities
    related_id      UUID             DEFAULT NULL,
    related_type    TEXT             DEFAULT NULL,

    CONSTRAINT account_notifications_related_type_length CHECK (
        related_type IS NULL OR length(trim(related_type)) BETWEEN 1 AND 50
    ),

    -- Additional data
    metadata        JSONB            NOT NULL DEFAULT '{}',

    CONSTRAINT account_notifications_metadata_size CHECK (length(metadata::TEXT) BETWEEN 2 AND 4096),

    -- Lifecycle timestamps
    created_at      TIMESTAMPTZ      NOT NULL DEFAULT current_timestamp,
    expires_at      TIMESTAMPTZ      DEFAULT NULL,

    CONSTRAINT account_notifications_expires_after_created CHECK (
        expires_at IS NULL OR expires_at > created_at
    ),
    CONSTRAINT account_notifications_read_after_created CHECK (
        read_at IS NULL OR read_at >= created_at
    )
);

-- Create indexes for account notifications
CREATE INDEX account_notifications_account_unread_idx
    ON account_notifications (account_id, created_at DESC)
    WHERE is_read = FALSE;

CREATE INDEX account_notifications_account_all_idx
    ON account_notifications (account_id, created_at DESC);

CREATE INDEX account_notifications_type_idx
    ON account_notifications (account_id, notify_type, created_at DESC)
    WHERE is_read = FALSE;

CREATE INDEX account_notifications_related_idx
    ON account_notifications (related_type, related_id)
    WHERE related_type IS NOT NULL AND related_id IS NOT NULL;

CREATE INDEX account_notifications_cleanup_idx
    ON account_notifications (expires_at)
    WHERE expires_at IS NOT NULL;

-- Add table and column comments
COMMENT ON TABLE account_notifications IS
    'User notifications for mentions, replies, invites, and system announcements.';

COMMENT ON COLUMN account_notifications.id IS 'Unique notification identifier';
COMMENT ON COLUMN account_notifications.account_id IS 'Account receiving the notification';
COMMENT ON COLUMN account_notifications.notify_type IS 'Type of notification';
COMMENT ON COLUMN account_notifications.title IS 'Notification title (1-200 chars)';
COMMENT ON COLUMN account_notifications.message IS 'Notification message (1-1000 chars)';
COMMENT ON COLUMN account_notifications.is_read IS 'Whether notification has been read';
COMMENT ON COLUMN account_notifications.read_at IS 'Timestamp when notification was read';
COMMENT ON COLUMN account_notifications.related_id IS 'ID of related entity (comment, document, etc.)';
COMMENT ON COLUMN account_notifications.related_type IS 'Type of related entity';
COMMENT ON COLUMN account_notifications.metadata IS 'Additional notification data (JSON, 2B-4KB)';
COMMENT ON COLUMN account_notifications.created_at IS 'Notification creation timestamp';
COMMENT ON COLUMN account_notifications.expires_at IS 'Optional expiration timestamp';

-- Create a view for active user sessions
CREATE VIEW active_user_sessions AS
SELECT
    t.id,
    t.account_id,
    a.email_address,
    a.display_name,
    t.ip_address,
    t.user_agent,
    t.session_type,
    t.is_remembered,
    t.issued_at,
    t.expired_at,
    t.last_used_at
FROM account_api_tokens t
    JOIN accounts a ON t.account_id = a.id
WHERE t.deleted_at IS NULL
    AND t.expired_at > current_timestamp
    AND a.deleted_at IS NULL;

COMMENT ON VIEW active_user_sessions IS
    'View of currently active user sessions with account information for monitoring and security purposes.';

-- Create a function to clean up expired sessions and tokens
CREATE OR REPLACE FUNCTION cleanup_expired_auth_data()
RETURNS TABLE (
    sessions_cleaned INTEGER,
    tokens_cleaned INTEGER
)
LANGUAGE plpgsql AS $$
DECLARE
    sessions_count INTEGER;
    tokens_count INTEGER;
BEGIN
    -- Clean up expired sessions
    WITH deleted_sessions AS (
        UPDATE account_api_tokens
        SET deleted_at = current_timestamp
        WHERE expired_at < current_timestamp
            AND deleted_at IS NULL
        RETURNING 1
    )
    SELECT count(*)
    INTO sessions_count
    FROM deleted_sessions;

    -- Clean up expired and used tokens
    WITH deleted_tokens AS (
        DELETE FROM account_action_tokens
        WHERE expired_at < current_timestamp
            OR used_at IS NOT NULL
        RETURNING 1
    )
    SELECT count(*)
    INTO tokens_count
    FROM deleted_tokens;

    -- Return cleanup results
    RETURN QUERY SELECT sessions_count, tokens_count;
END;
$$;

COMMENT ON FUNCTION cleanup_expired_auth_data() IS
    'Cleans up expired sessions and tokens. Returns count of cleaned records.';

-- Create cleanup function for expired notifications
CREATE OR REPLACE FUNCTION cleanup_expired_notifications()
RETURNS INTEGER
LANGUAGE plpgsql AS $$
DECLARE
    deleted_count INTEGER := 0;
BEGIN
    -- Delete expired notifications
    WITH deleted AS (
        DELETE FROM account_notifications
        WHERE expires_at IS NOT NULL
          AND expires_at < CURRENT_TIMESTAMP
        RETURNING id
    )
    SELECT COUNT(*)
    INTO deleted_count
    FROM deleted;

    RETURN deleted_count;
END;
$$;

COMMENT ON FUNCTION cleanup_expired_notifications() IS
    'Deletes expired notifications. Returns count of deleted notifications.';
