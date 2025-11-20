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
    phone_number          TEXT        DEFAULT NULL,
    avatar_url            TEXT        DEFAULT NULL,

    CONSTRAINT accounts_company_name_length_max CHECK (company_name IS NULL OR length(company_name) <= 255),
    CONSTRAINT accounts_phone_number_length_max CHECK (phone_number IS NULL OR length(phone_number) <= 50),

    -- Preferences and settings
    timezone              TEXT        NOT NULL DEFAULT 'UTC',
    locale                TEXT        NOT NULL DEFAULT 'en-US',

    CONSTRAINT accounts_timezone_format CHECK (timezone ~ '^[A-Za-z_/]+$'),
    CONSTRAINT accounts_locale_format CHECK (locale ~ '^[a-z]{2}-[A-Z]{2}$'),

    -- Security tracking
    failed_login_attempts INTEGER     NOT NULL DEFAULT 0,
    locked_until          TIMESTAMPTZ DEFAULT NULL,
    password_changed_at   TIMESTAMPTZ DEFAULT NULL,

    CONSTRAINT accounts_failed_login_attempts_range CHECK (failed_login_attempts BETWEEN 0 AND 10),
    CONSTRAINT accounts_locked_until_future CHECK (locked_until IS NULL OR locked_until > current_timestamp),

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

CREATE INDEX accounts_security_tracking_idx
    ON accounts (failed_login_attempts, locked_until)
    WHERE deleted_at IS NULL AND (failed_login_attempts > 0 OR locked_until IS NOT NULL);

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
COMMENT ON COLUMN accounts.phone_number IS 'Optional phone number for 2FA or emergency contact';
COMMENT ON COLUMN accounts.avatar_url IS 'URL to user profile image or avatar';
COMMENT ON COLUMN accounts.timezone IS 'User timezone for date/time display preferences';
COMMENT ON COLUMN accounts.locale IS 'User locale for language and regional formatting';
COMMENT ON COLUMN accounts.failed_login_attempts IS 'Counter for consecutive failed login attempts (0-10)';
COMMENT ON COLUMN accounts.locked_until IS 'Temporary account lock expiration after too many failed logins';
COMMENT ON COLUMN accounts.password_changed_at IS 'Timestamp of last password change for security tracking';
COMMENT ON COLUMN accounts.created_at IS 'Timestamp when the account was created';
COMMENT ON COLUMN accounts.updated_at IS 'Timestamp when the account was last modified (auto-updated by trigger)';
COMMENT ON COLUMN accounts.deleted_at IS 'Timestamp when the account was soft-deleted (NULL if active)';

-- Create enhanced account API tokens table
CREATE TABLE account_api_tokens (
    -- Session identifiers
    access_seq            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    refresh_seq           UUID        NOT NULL DEFAULT gen_random_uuid(),

    -- Account reference
    account_id            UUID        NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,

    -- Token metadata
    name                  TEXT        NOT NULL,
    description           TEXT        DEFAULT NULL,

    CONSTRAINT account_api_tokens_name_not_empty CHECK (trim(name) <> ''),
    CONSTRAINT account_api_tokens_name_length CHECK (length(name) <= 100),
    CONSTRAINT account_api_tokens_description_length CHECK (description IS NULL OR length(description) <= 500),

    -- Geographic and device tracking
    region_code           CHAR(2)     NOT NULL DEFAULT 'XX',
    country_code          CHAR(2)     DEFAULT NULL,
    city_name             TEXT        DEFAULT NULL,

    CONSTRAINT account_api_tokens_region_code_valid CHECK (region_code ~ '^[A-Z0-9]{2}$'),
    CONSTRAINT account_api_tokens_country_code_valid CHECK (country_code IS NULL OR country_code ~ '^[A-Z]{2}$'),

    -- Security context
    ip_address            INET        NOT NULL,
    user_agent            TEXT        NOT NULL,
    device_id             TEXT        DEFAULT NULL,
    session_type          API_TOKEN_TYPE NOT NULL DEFAULT 'web',

    CONSTRAINT account_api_tokens_user_agent_not_empty CHECK (trim(user_agent) <> ''),

    -- Security flags
    is_suspicious         BOOLEAN     NOT NULL DEFAULT FALSE,
    is_remembered         BOOLEAN     NOT NULL DEFAULT FALSE,

    -- Session lifecycle
    issued_at             TIMESTAMPTZ NOT NULL DEFAULT current_timestamp,
    expired_at            TIMESTAMPTZ NOT NULL DEFAULT current_timestamp + INTERVAL '7 days',
    last_used_at          TIMESTAMPTZ DEFAULT NULL,
    deleted_at            TIMESTAMPTZ DEFAULT NULL,

    CONSTRAINT account_api_tokens_expired_after_issued CHECK (expired_at > issued_at),
    CONSTRAINT account_api_tokens_deleted_after_issued CHECK (deleted_at IS NULL OR deleted_at >= issued_at),
    CONSTRAINT account_api_tokens_last_used_after_issued CHECK (last_used_at IS NULL OR last_used_at >= issued_at)
);

-- Create indexes for API token management
CREATE UNIQUE INDEX account_api_tokens_access_seq_unique_idx
    ON account_api_tokens (access_seq);

CREATE UNIQUE INDEX account_api_tokens_refresh_seq_unique_idx
    ON account_api_tokens (refresh_seq);

CREATE INDEX account_api_tokens_account_active_idx
    ON account_api_tokens (account_id, access_seq, expired_at)
    WHERE deleted_at IS NULL;

CREATE INDEX account_api_tokens_account_refresh_idx
    ON account_api_tokens (account_id, refresh_seq, expired_at)
    WHERE deleted_at IS NULL;

CREATE INDEX account_api_tokens_cleanup_idx
    ON account_api_tokens (expired_at)
    WHERE deleted_at IS NULL;

CREATE INDEX account_api_tokens_device_tracking_idx
    ON account_api_tokens (account_id, device_id, issued_at DESC)
    WHERE device_id IS NOT NULL AND deleted_at IS NULL;

CREATE INDEX account_api_tokens_geographic_idx
    ON account_api_tokens (country_code, region_code, issued_at DESC)
    WHERE country_code IS NOT NULL AND deleted_at IS NULL;

-- Add table and column comments
COMMENT ON TABLE account_api_tokens IS
    'User authentication sessions with enhanced security tracking and geographic information.';

COMMENT ON COLUMN account_api_tokens.access_seq IS 'Unique session identifier used for authentication (UUID)';
COMMENT ON COLUMN account_api_tokens.refresh_seq IS 'Unique refresh token for extending session without re-authentication (UUID)';
COMMENT ON COLUMN account_api_tokens.account_id IS 'Reference to the account this session belongs to';
COMMENT ON COLUMN account_api_tokens.name IS 'Human-readable name for the API token (max 100 characters)';
COMMENT ON COLUMN account_api_tokens.description IS 'Optional description for the API token (max 500 characters)';
COMMENT ON COLUMN account_api_tokens.region_code IS 'Two-character region/state code where session originated';
COMMENT ON COLUMN account_api_tokens.country_code IS 'ISO 3166-1 alpha-2 country code where session originated';
COMMENT ON COLUMN account_api_tokens.city_name IS 'City name where session originated (if available from IP geolocation)';
COMMENT ON COLUMN account_api_tokens.ip_address IS 'IP address from which the session was initiated';
COMMENT ON COLUMN account_api_tokens.user_agent IS 'Browser/client user agent string for device identification';
COMMENT ON COLUMN account_api_tokens.device_id IS 'Optional persistent device identifier for trusted device tracking';
COMMENT ON COLUMN account_api_tokens.session_type IS 'Type of client that created the session (web, mobile, api, desktop)';
COMMENT ON COLUMN account_api_tokens.is_suspicious IS 'Flag indicating potentially suspicious session activity';
COMMENT ON COLUMN account_api_tokens.is_remembered IS 'Flag indicating if this is a "remember me" extended session';
COMMENT ON COLUMN account_api_tokens.issued_at IS 'Timestamp when the session was created';
COMMENT ON COLUMN account_api_tokens.expired_at IS 'Timestamp when the session expires and becomes invalid';
COMMENT ON COLUMN account_api_tokens.last_used_at IS 'Timestamp of most recent session activity';
COMMENT ON COLUMN account_api_tokens.deleted_at IS 'Timestamp when the session was soft-deleted (NULL if active)';

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
    ip_address            INET        NOT NULL,
    user_agent            TEXT        NOT NULL,
    device_id             TEXT        DEFAULT NULL,

    CONSTRAINT account_action_tokens_user_agent_not_empty CHECK (trim(user_agent) <> ''),

    -- Rate limiting and security
    attempt_count         INTEGER     NOT NULL DEFAULT 0,
    max_attempts          INTEGER     NOT NULL DEFAULT 3,

    CONSTRAINT account_action_tokens_attempt_count_range CHECK (attempt_count BETWEEN 0 AND max_attempts),
    CONSTRAINT account_action_tokens_max_attempts_range CHECK (max_attempts BETWEEN 1 AND 10),

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

CREATE INDEX account_action_tokens_security_monitoring_idx
    ON account_action_tokens (ip_address, attempt_count, issued_at)
    WHERE attempt_count > 0;

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
COMMENT ON COLUMN account_action_tokens.attempt_count IS 'Number of times this token has been attempted (for rate limiting)';
COMMENT ON COLUMN account_action_tokens.max_attempts IS 'Maximum allowed attempts before token becomes invalid';
COMMENT ON COLUMN account_action_tokens.issued_at IS 'Timestamp when the token was created';
COMMENT ON COLUMN account_action_tokens.expired_at IS 'Timestamp after which the token becomes invalid';
COMMENT ON COLUMN account_action_tokens.used_at IS 'Timestamp when the token was successfully used (NULL if unused)';

-- Create a view for active user sessions
CREATE VIEW active_user_sessions AS
SELECT
    t.access_seq,
    t.account_id,
    a.email_address,
    a.display_name,
    t.ip_address,
    t.country_code,
    t.region_code,
    t.session_type,
    t.issued_at,
    t.expired_at,
    t.last_used_at,
    t.is_suspicious
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

-- Create notification type enum
CREATE TYPE NOTIFICATION_TYPE AS ENUM (
    'comment_mention',      -- User was mentioned in a comment
    'comment_reply',        -- Someone replied to user's comment
    'document_upload',      -- Document was uploaded
    'document_download',    -- Document was downloaded
    'document_verify',      -- Document verification completed
    'project_invite',       -- User was invited to a project
    'system_announcement'   -- System-wide announcement
);

COMMENT ON TYPE NOTIFICATION_TYPE IS
    'Types of notifications that can be sent to users.';

-- Create account notifications table
CREATE TABLE account_notifications (
    -- Primary identifiers
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References
    account_id      UUID             NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,

    -- Notification details
    notify_type     NOTIFICATION_TYPE NOT NULL,
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

-- Set up automatic updated_at trigger (if needed)
-- Note: Notifications typically don't need updated_at since they're read-only after creation

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
