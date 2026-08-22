-- Event outbox: the transactional-outbox table for workspace events. A write
-- action inserts one row here in the same transaction as the action, so the two
-- commit or roll back together; a background drainer then projects each pending
-- row onto its sinks (activity log, webhooks, notifications) and marks it done.

-- Processing state of an outbox row: what the drainer has done with it.
CREATE TYPE OUTBOX_STATUS AS ENUM (
    'pending',      -- Awaiting projection (or deferred for a later retry)
    'processed',    -- Durably projected to its sinks
    'failed'        -- Given up on after too many failed attempts (dead-lettered)
);

COMMENT ON TYPE OUTBOX_STATUS IS 'Processing state of an event-outbox row.';

-- Event outbox table: one workspace event awaiting or past projection.
CREATE TABLE event_outbox (
    -- Primary identifier
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References
    workspace_id  UUID          NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    account_id    UUID          NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,

    -- The event. A serialized `WorkspaceEvent`: its variant tag and typed facts.
    -- The drainer decodes this to project the event onto its sinks.
    event         JSONB         NOT NULL,
    CONSTRAINT event_outbox_event_size CHECK (length(event::TEXT) BETWEEN 2 AND 16384),

    -- Context tracking, carried through to the activity-log entry the drainer writes.
    ip_address    INET          DEFAULT NULL,
    user_agent    TEXT          DEFAULT NULL,

    -- Drainer bookkeeping: the row's processing state, how many delivery attempts
    -- it has taken, and the earliest time it may next be claimed (advanced by a
    -- backoff on each failed attempt so a failing row does not spin at the head of
    -- the queue).
    status          OUTBOX_STATUS NOT NULL DEFAULT 'pending',
    attempts        INTEGER       NOT NULL DEFAULT 0,
    CONSTRAINT event_outbox_attempts_non_negative CHECK (attempts >= 0),
    next_attempt_at TIMESTAMPTZ   NOT NULL DEFAULT current_timestamp,

    -- Lifecycle timestamps
    created_at    TIMESTAMPTZ     NOT NULL DEFAULT current_timestamp,

    -- When a terminal row (processed or failed) was resolved by an operator; NULL
    -- until then. A manual affordance for inspecting the outbox after the fact.
    resolved_at   TIMESTAMPTZ     DEFAULT NULL,
    CONSTRAINT event_outbox_resolved_only_when_terminal
        CHECK (resolved_at IS NULL OR status IN ('processed', 'failed')),
    CONSTRAINT event_outbox_resolved_after_created
        CHECK (resolved_at IS NULL OR resolved_at >= created_at)
);

-- The drainer's claim queue: pending rows ordered by due time then age, so a
-- batch claims the oldest due rows. Partial so it stays small as processed and
-- failed rows accumulate.
CREATE INDEX event_outbox_pending_idx
    ON event_outbox (next_attempt_at, created_at)
    WHERE status = 'pending';

COMMENT ON TABLE event_outbox IS 'Transactional outbox of workspace events, drained to the activity log, webhooks, and notifications.';
COMMENT ON COLUMN event_outbox.id IS 'Unique outbox row identifier';
COMMENT ON COLUMN event_outbox.workspace_id IS 'Workspace the event was raised in';
COMMENT ON COLUMN event_outbox.account_id IS 'Account that performed the action';
COMMENT ON COLUMN event_outbox.event IS 'Serialized WorkspaceEvent; its variant tag and typed facts (JSON, 2B-16KB)';
COMMENT ON COLUMN event_outbox.ip_address IS 'IP address where the action originated';
COMMENT ON COLUMN event_outbox.user_agent IS 'User agent of the client';
COMMENT ON COLUMN event_outbox.status IS 'Processing state: pending, processed, or failed (dead-lettered)';
COMMENT ON COLUMN event_outbox.attempts IS 'Number of delivery attempts the drainer has made';
COMMENT ON COLUMN event_outbox.next_attempt_at IS 'Earliest time the row may next be claimed; advanced by a backoff after each failed attempt';
COMMENT ON COLUMN event_outbox.resolved_at IS 'When a terminal (processed or failed) row was resolved by an operator; NULL until then. A manual affordance for inspecting the outbox after the fact';
COMMENT ON COLUMN event_outbox.created_at IS 'Timestamp when the event was raised';
