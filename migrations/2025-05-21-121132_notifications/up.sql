-- Notifications: per-account notification inbox (mentions, replies,
-- invites, system announcements). Account-scoped but a standalone feature.

-- Create notification event enum
CREATE TYPE NOTIFICATION_EVENT AS ENUM (
    -- Member events
    'member.invited',            -- User was invited to a workspace
    'member.joined',             -- A new member joined a workspace

    -- Connection sync events
    'connection.sync.completed', -- A connection sync completed
    'connection.sync.failed',    -- A connection sync failed

    -- Pipeline run events
    'pipeline.run.analyzed',     -- A run finished detection, awaiting review
    'pipeline.run.completed',    -- A run completed (redaction produced)
    'pipeline.run.failed',       -- A run failed

    -- System events
    'system.announcement',       -- System-wide announcement
    'system.report'              -- System report generated
);

COMMENT ON TYPE NOTIFICATION_EVENT IS
    'Types of notification events that can be sent to users.';

-- Create account notifications table
CREATE TABLE account_notifications (
    -- Primary identifiers
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References
    account_id      UUID             NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,

    -- Notification details. The type is the client-side localization key; the
    -- params carry its typed fields (a `notifyType`-tagged payload). No rendered
    -- text is stored — the client renders the copy from the type and params.
    notify_type     NOTIFICATION_EVENT NOT NULL,

    -- Read state: `read_at IS NULL` means unread; a timestamp means read.
    read_at         TIMESTAMPTZ      DEFAULT NULL,

    -- Typed params for the notification's type (the tagged payload's fields).
    params          JSONB            NOT NULL DEFAULT '{}',

    CONSTRAINT account_notifications_params_size CHECK (length(params::TEXT) BETWEEN 2 AND 4096),

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
    WHERE read_at IS NULL;

CREATE INDEX account_notifications_account_all_idx
    ON account_notifications (account_id, created_at DESC);

CREATE INDEX account_notifications_type_idx
    ON account_notifications (account_id, notify_type, created_at DESC)
    WHERE read_at IS NULL;

CREATE INDEX account_notifications_cleanup_idx
    ON account_notifications (expires_at)
    WHERE expires_at IS NOT NULL;

-- Add table and column comments
COMMENT ON TABLE account_notifications IS
    'User notifications for mentions, replies, invites, and system announcements.';

COMMENT ON COLUMN account_notifications.id IS 'Unique notification identifier';
COMMENT ON COLUMN account_notifications.account_id IS 'Account receiving the notification';
COMMENT ON COLUMN account_notifications.notify_type IS 'Notification type; the client-side localization key';
COMMENT ON COLUMN account_notifications.read_at IS 'When the notification was read; NULL means unread';
COMMENT ON COLUMN account_notifications.params IS 'Typed params for the notification type (JSON, 2B-4KB)';
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
