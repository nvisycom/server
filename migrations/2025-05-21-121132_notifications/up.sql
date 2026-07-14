-- Notifications: per-account notification inbox (mentions, replies,
-- invites, system announcements). Account-scoped but a standalone feature.

-- Create notification event enum
CREATE TYPE NOTIFICATION_EVENT AS ENUM (
    -- File events
    'file:uploaded',          -- File was uploaded
    'file:downloaded',        -- File was downloaded
    'file:verified',          -- File verification completed

    -- Member events
    'member:invited',         -- User was invited to a workspace
    'member:joined',          -- A new member joined a workspace

    -- Connection events
    'connection:synced',      -- Connection sync completed
    'connection:desynced',    -- Connection sync failed or disconnected

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
