-- Event outbox: the transactional-outbox table for workspace events. A write
-- action inserts one row here in the same transaction as the action, so the two
-- commit or roll back together; a background drainer then projects each pending
-- row onto its sinks (activity log, webhooks, notifications) and marks it done.

-- Event outbox table: one pending workspace event awaiting projection.
CREATE TABLE event_outbox (
    -- Primary identifier
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References
    workspace_id  UUID        NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    account_id    UUID        NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,

    -- The event. A serialized `WorkspaceEvent`: its variant tag and typed facts.
    -- The drainer decodes this to project the event onto its sinks.
    event         JSONB       NOT NULL,
    CONSTRAINT event_outbox_event_size CHECK (length(event::TEXT) BETWEEN 2 AND 16384),

    -- Context tracking, carried through to the activity-log entry the drainer writes.
    ip_address    INET        DEFAULT NULL,
    user_agent    TEXT        DEFAULT NULL,

    -- Drainer bookkeeping: when processing finished (NULL while pending), how many
    -- delivery attempts the row has taken, the earliest time it may next be claimed
    -- (advanced by a backoff on each failed attempt so a failing row does not spin
    -- at the head of the queue), and when it was given up on after too many failed
    -- attempts (a poison row the drainer stops retrying).
    processed_at  TIMESTAMPTZ DEFAULT NULL,
    attempts      INTEGER     NOT NULL DEFAULT 0,
    CONSTRAINT event_outbox_attempts_non_negative CHECK (attempts >= 0),
    next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT current_timestamp,
    failed_at     TIMESTAMPTZ DEFAULT NULL,

    -- Lifecycle timestamp
    created_at    TIMESTAMPTZ NOT NULL DEFAULT current_timestamp
);

-- The drainer's claim queue: pending (unprocessed, not given up on) rows ordered
-- by due time then age, so a batch claims the oldest due rows. Partial so it stays
-- small as processed and failed rows accumulate.
CREATE INDEX event_outbox_pending_idx
    ON event_outbox (next_attempt_at, created_at)
    WHERE processed_at IS NULL AND failed_at IS NULL;

COMMENT ON TABLE event_outbox IS 'Transactional outbox of workspace events, drained to the activity log, webhooks, and notifications.';
COMMENT ON COLUMN event_outbox.id IS 'Unique outbox row identifier';
COMMENT ON COLUMN event_outbox.workspace_id IS 'Workspace the event was raised in';
COMMENT ON COLUMN event_outbox.account_id IS 'Account that performed the action';
COMMENT ON COLUMN event_outbox.event IS 'Serialized WorkspaceEvent; its variant tag and typed facts (JSON, 2B-16KB)';
COMMENT ON COLUMN event_outbox.ip_address IS 'IP address where the action originated';
COMMENT ON COLUMN event_outbox.user_agent IS 'User agent of the client';
COMMENT ON COLUMN event_outbox.processed_at IS 'Timestamp when the drainer finished projecting the event; NULL while pending';
COMMENT ON COLUMN event_outbox.attempts IS 'Number of delivery attempts the drainer has made';
COMMENT ON COLUMN event_outbox.next_attempt_at IS 'Earliest time the row may next be claimed; advanced by a backoff after each failed attempt';
COMMENT ON COLUMN event_outbox.failed_at IS 'Timestamp the drainer gave up on the row after too many failed attempts; NULL unless dead-lettered';
COMMENT ON COLUMN event_outbox.created_at IS 'Timestamp when the event was raised';
