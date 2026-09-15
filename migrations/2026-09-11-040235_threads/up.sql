-- Threads: threaded discussion modeled after GitHub issues. A thread is a pure
-- discussion primitive — an author, an optional title, an open/closed lifecycle,
-- and a stream of comments and (discussion-only) timeline events. It is not, by
-- itself, about any resource. A document review (a later migration) attaches a
-- thread to a document; that subsystem owns its own activity log, kept separate
-- from this discussion timeline.

-- Threads: a discussion, closable and soft-deletable. A thread is not about any
-- resource on its own; a workspace_reviews row attaches one to a document.
CREATE TABLE workspace_threads (
    -- Primary identifier
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- The workspace this thread belongs to.
    workspace_id        UUID        NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,

    -- The account that opened the thread. If it is removed, the thread goes too.
    author_account_id   UUID        NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,

    -- Optional human-readable title (like a GitHub issue title); NULL for an
    -- untitled thread. Renaming it records a thread.renamed timeline event.
    display_name        TEXT        DEFAULT NULL,
    CONSTRAINT workspace_threads_display_name_length CHECK (display_name IS NULL OR length(trim(display_name)) BETWEEN 1 AND 255),

    -- Lifecycle state (open/closed). `closed_at IS NULL` means open; a timestamp
    -- means closed, and `closed_by` records who closed it (SET NULL if that account
    -- is removed). This is the current state; the per-transition history lives in
    -- workspace_thread_events.
    closed_at           TIMESTAMPTZ DEFAULT NULL,
    closed_by           UUID        DEFAULT NULL REFERENCES accounts (id) ON DELETE SET NULL,
    CONSTRAINT workspace_threads_closed_consistent CHECK (
        (closed_at IS NULL) = (closed_by IS NULL)
    ),

    -- Lifecycle timestamps
    created_at          TIMESTAMPTZ NOT NULL DEFAULT current_timestamp,
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT current_timestamp,
    deleted_at          TIMESTAMPTZ DEFAULT NULL,
    CONSTRAINT workspace_threads_updated_after_created CHECK (updated_at >= created_at),
    CONSTRAINT workspace_threads_deleted_after_created CHECK (deleted_at IS NULL OR deleted_at >= created_at),
    CONSTRAINT workspace_threads_closed_after_created CHECK (closed_at IS NULL OR closed_at >= created_at)
);

-- Workspace-scoped thread listing, newest first, filterable by open/closed.
CREATE INDEX workspace_threads_workspace_idx
    ON workspace_threads (workspace_id, created_at DESC)
    WHERE deleted_at IS NULL;

-- Keep updated_at current on every row modification.
SELECT setup_updated_at('workspace_threads');

COMMENT ON TABLE workspace_threads IS 'A discussion thread: an author, an optional title, an open/closed lifecycle, and a stream of comments and events. A workspace_reviews row attaches a thread to a document review.';
COMMENT ON COLUMN workspace_threads.id IS 'Unique thread identifier';
COMMENT ON COLUMN workspace_threads.workspace_id IS 'Denormalized workspace scope for fast per-workspace thread queries';
COMMENT ON COLUMN workspace_threads.author_account_id IS 'Account that opened the thread';
COMMENT ON COLUMN workspace_threads.display_name IS 'Optional human-readable title; NULL for an untitled thread (1-255 chars)';
COMMENT ON COLUMN workspace_threads.closed_at IS 'When the thread was closed; NULL means open';
COMMENT ON COLUMN workspace_threads.closed_by IS 'Account that closed the thread; null if open or that account was removed';
COMMENT ON COLUMN workspace_threads.created_at IS 'Timestamp when the thread was opened';
COMMENT ON COLUMN workspace_threads.updated_at IS 'Timestamp of the last update';
COMMENT ON COLUMN workspace_threads.deleted_at IS 'Soft-deletion timestamp; NULL means live';

-- Thread comments: one message within a thread.
CREATE TABLE workspace_thread_comments (
    -- Primary identifier
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- The comment this one answers, when it is a reply: for an assistant reply,
    -- the message that addressed the assistant; NULL for an ordinary message. A
    -- unique index below allows at most one live reply per parent, so a
    -- redelivered assistant job cannot post a second reply.
    parent_id           UUID        DEFAULT NULL REFERENCES workspace_thread_comments (id) ON DELETE SET NULL,

    -- References. Denormalized workspace scope for fast per-workspace queries, and
    -- the thread this message belongs to; deleting the thread removes its comments
    -- (CASCADE), so closing/deleting a discussion takes its messages.
    workspace_id        UUID        NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    thread_id           UUID        NOT NULL REFERENCES workspace_threads (id) ON DELETE CASCADE,

    -- The message author. If their account is removed, their comments go with it.
    author_account_id   UUID        NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,

    -- The message text.
    body                TEXT        NOT NULL,
    CONSTRAINT workspace_thread_comments_body_length CHECK (length(trim(body)) BETWEEN 1 AND 10000),

    -- Lifecycle timestamps
    created_at          TIMESTAMPTZ NOT NULL DEFAULT current_timestamp,
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT current_timestamp,
    deleted_at          TIMESTAMPTZ DEFAULT NULL,
    CONSTRAINT workspace_thread_comments_updated_after_created CHECK (updated_at >= created_at),
    CONSTRAINT workspace_thread_comments_deleted_after_created CHECK (deleted_at IS NULL OR deleted_at >= created_at)
);

-- A thread's messages, oldest first (a discussion reads top to bottom).
CREATE INDEX workspace_thread_comments_thread_idx
    ON workspace_thread_comments (thread_id, created_at)
    WHERE deleted_at IS NULL;

-- At most one live reply per parent comment: a redelivered assistant job
-- re-inserting its reply hits this and is treated as already answered, so the
-- assistant never double-replies to one mention.
CREATE UNIQUE INDEX workspace_thread_comments_parent_unique_idx
    ON workspace_thread_comments (parent_id)
    WHERE parent_id IS NOT NULL AND deleted_at IS NULL;

-- Keep updated_at current on every row modification.
SELECT setup_updated_at('workspace_thread_comments');

COMMENT ON TABLE workspace_thread_comments IS 'One message within a thread.';
COMMENT ON COLUMN workspace_thread_comments.id IS 'Unique comment identifier';
COMMENT ON COLUMN workspace_thread_comments.parent_id IS 'For a reply, the comment it answers; NULL otherwise (one live reply per parent)';
COMMENT ON COLUMN workspace_thread_comments.workspace_id IS 'Denormalized workspace scope';
COMMENT ON COLUMN workspace_thread_comments.thread_id IS 'Thread this message belongs to';
COMMENT ON COLUMN workspace_thread_comments.author_account_id IS 'Account that wrote the message';
COMMENT ON COLUMN workspace_thread_comments.body IS 'Message text (1-10000 chars)';
COMMENT ON COLUMN workspace_thread_comments.created_at IS 'Timestamp when the comment was posted';
COMMENT ON COLUMN workspace_thread_comments.updated_at IS 'Timestamp of the last edit';
COMMENT ON COLUMN workspace_thread_comments.deleted_at IS 'Soft-deletion timestamp; NULL means live';

-- Kind of a thread timeline event (a non-message entry in a thread's stream):
-- the discussion lifecycle only (opened/closed/reopened/renamed).
CREATE TYPE THREAD_EVENT_KIND AS ENUM (
    'thread.opened',   -- The thread was opened
    'thread.closed',   -- The thread was closed
    'thread.reopened', -- The thread was reopened
    'thread.renamed'   -- The thread's display name was changed
);

COMMENT ON TYPE THREAD_EVENT_KIND IS 'The kind of a non-message entry in a thread timeline: the discussion lifecycle (opened/closed/reopened/renamed). Review activity lives in workspace_review_events.';

-- Thread timeline events: the non-message entries in a thread's stream (the
-- discussion lifecycle). Immutable — an event is a fact that happened, so there is
-- no update or soft-delete; deleting the thread removes them. Review activity lives
-- in workspace_review_events, not here.
CREATE TABLE workspace_thread_events (
    -- Primary identifier
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References. Denormalized workspace scope (matching the sibling tables) and
    -- the thread this event belongs to.
    workspace_id        UUID        NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    thread_id           UUID        NOT NULL REFERENCES workspace_threads (id) ON DELETE CASCADE,

    -- What happened.
    kind                THREAD_EVENT_KIND NOT NULL,

    -- Who did it. If their account is removed, keep the event but forget the actor
    -- (the transition still happened).
    actor_account_id    UUID        DEFAULT NULL REFERENCES accounts (id) ON DELETE SET NULL,

    -- Event-specific detail, when any: the new name for a rename. NULL otherwise.
    target              JSONB       DEFAULT NULL,
    CONSTRAINT workspace_thread_events_target_size CHECK (target IS NULL OR length(target::TEXT) <= 8192),

    -- When it happened (events are immutable, so only a creation timestamp).
    created_at          TIMESTAMPTZ NOT NULL DEFAULT current_timestamp
);

-- A thread's timeline events, oldest first (merged with comments by the reader).
CREATE INDEX workspace_thread_events_thread_idx
    ON workspace_thread_events (thread_id, created_at);

COMMENT ON TABLE workspace_thread_events IS 'An immutable non-message entry in a thread timeline: the discussion lifecycle (opened/closed/reopened/renamed).';
COMMENT ON COLUMN workspace_thread_events.id IS 'Unique event identifier';
COMMENT ON COLUMN workspace_thread_events.workspace_id IS 'Denormalized workspace scope';
COMMENT ON COLUMN workspace_thread_events.thread_id IS 'Thread this event belongs to';
COMMENT ON COLUMN workspace_thread_events.kind IS 'What happened (thread.opened/closed/reopened/renamed)';
COMMENT ON COLUMN workspace_thread_events.actor_account_id IS 'Account that performed the action; null if that account was removed';
COMMENT ON COLUMN workspace_thread_events.target IS 'Event-specific detail (the new name for a rename); NULL when none';
COMMENT ON COLUMN workspace_thread_events.created_at IS 'Timestamp when the event happened';

-- Thread lifecycle events feed the event sinks; each value is added by this
-- migration (the one that introduces threads). ALTER TYPE only adds labels here
-- (no rows use them yet), so it stays transactional.
--
-- Activity log records the thread lifecycle plus each message posted.
ALTER TYPE ACTIVITY_TYPE ADD VALUE IF NOT EXISTS 'thread.opened';
ALTER TYPE ACTIVITY_TYPE ADD VALUE IF NOT EXISTS 'thread.closed';
ALTER TYPE ACTIVITY_TYPE ADD VALUE IF NOT EXISTS 'thread.reopened';
ALTER TYPE ACTIVITY_TYPE ADD VALUE IF NOT EXISTS 'thread.renamed';
ALTER TYPE ACTIVITY_TYPE ADD VALUE IF NOT EXISTS 'thread.deleted';
ALTER TYPE ACTIVITY_TYPE ADD VALUE IF NOT EXISTS 'thread.comment.created';

-- Webhooks carry the thread lifecycle (message-level noise is left off).
ALTER TYPE WEBHOOK_EVENT ADD VALUE IF NOT EXISTS 'thread.opened';
ALTER TYPE WEBHOOK_EVENT ADD VALUE IF NOT EXISTS 'thread.closed';
ALTER TYPE WEBHOOK_EVENT ADD VALUE IF NOT EXISTS 'thread.reopened';
ALTER TYPE WEBHOOK_EVENT ADD VALUE IF NOT EXISTS 'thread.renamed';

-- In-app notifications go to each mentioned account.
ALTER TYPE NOTIFICATION_EVENT ADD VALUE IF NOT EXISTS 'comment.mentioned';
