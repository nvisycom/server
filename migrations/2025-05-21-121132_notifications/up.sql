-- Notifications: per-account notification inbox for member, connection-sync,
-- and pipeline-run events. Account-scoped but a standalone feature; the client
-- renders copy from the event type and its typed params.

-- Type of a notification event: what happened that the account is told about.
CREATE TYPE NOTIFICATION_EVENT AS ENUM (
    'member.invited',            -- User was invited to a workspace
    'member.joined',             -- A new member joined a workspace

    'connection.sync.completed', -- A connection sync completed
    'connection.sync.failed',    -- A connection sync failed

    'pipeline.run.analyzed',     -- A run finished detection, awaiting review
    'pipeline.run.completed',    -- A run completed (redaction produced)
    'pipeline.run.failed'        -- A run failed
);

COMMENT ON TYPE NOTIFICATION_EVENT IS 'Type of a notification event delivered to an account.';

-- Account notifications table: one notification in an account's inbox.
CREATE TABLE account_notifications (
    -- Primary identifier
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References
    account_id      UUID             NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,

    -- The event type is the client-side localization key; the params carry its
    -- typed fields (a `notifyType`-tagged payload). No rendered text is stored —
    -- the client renders the copy from the type and params.
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

-- The unread inbox per account, newest first (the default badge/list query).
CREATE INDEX account_notifications_account_unread_idx
    ON account_notifications (account_id, created_at DESC)
    WHERE read_at IS NULL;

-- The full inbox per account, newest first (read and unread history).
CREATE INDEX account_notifications_account_all_idx
    ON account_notifications (account_id, created_at DESC);

-- Unread notifications of a given type per account (filtered inbox views).
CREATE INDEX account_notifications_type_idx
    ON account_notifications (account_id, notify_type, created_at DESC)
    WHERE read_at IS NULL;

-- Expiring notifications, to drive the cleanup sweep.
CREATE INDEX account_notifications_cleanup_idx
    ON account_notifications (expires_at)
    WHERE expires_at IS NOT NULL;

COMMENT ON TABLE account_notifications IS 'Per-account notification inbox for member, sync, pipeline, and system events.';
COMMENT ON COLUMN account_notifications.id IS 'Unique notification identifier';
COMMENT ON COLUMN account_notifications.account_id IS 'Account receiving the notification';
COMMENT ON COLUMN account_notifications.notify_type IS 'Event type; the client-side localization key';
COMMENT ON COLUMN account_notifications.read_at IS 'When the notification was read; NULL means unread';
COMMENT ON COLUMN account_notifications.params IS 'Typed params for the event type (JSON, 2B-4KB)';
COMMENT ON COLUMN account_notifications.created_at IS 'Notification creation timestamp';
COMMENT ON COLUMN account_notifications.expires_at IS 'Optional expiration timestamp; NULL means it does not expire';
