-- Threads: threaded discussion on a file under review, modeled after GitHub
-- issues. A thread is the closable, optionally file-anchored unit; its stream
-- interleaves comments (messages) and events (opened/closed/reopened, anchor
-- added or removed). A thread carries zero or more anchors — pins to locations
-- within its file — added and removed over its lifetime. Opening, closing,
-- reopening, and anchor changes are recorded both as in-thread timeline events
-- and as workspace events (activity log + webhooks).

-- Kind of a thread timeline event (a non-message entry in a thread's stream).
CREATE TYPE THREAD_EVENT_KIND AS ENUM (
    'thread.opened',        -- The thread was opened
    'thread.closed',        -- The thread was closed
    'thread.reopened',      -- The thread was reopened
    'thread.renamed',       -- The thread's display name was changed
    'thread.anchor.added',  -- An anchor (location pin) was added to the thread
    'thread.anchor.removed' -- An anchor was removed from the thread
);

COMMENT ON TYPE THREAD_EVENT_KIND IS 'The kind of a non-message entry in a thread timeline: opened, closed, reopened, renamed, or an anchor added/removed.';

-- Threads: the closable, optionally file-anchored unit of discussion.
CREATE TABLE workspace_threads (
    -- Primary identifier
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References. A thread belongs to a workspace and is optionally about one of
    -- its files: `file_id` NULL is a workspace-level discussion, a set `file_id`
    -- pins it to that file. Anchors (locations within the file) live in
    -- workspace_thread_anchors, since a thread may carry several.
    workspace_id        UUID        NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    file_id             UUID        DEFAULT NULL,

    -- The account that opened the thread. If it is removed, the thread goes too.
    author_account_id   UUID        NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,

    -- Optional human-readable title (like a GitHub issue title); NULL for an
    -- untitled thread. Renaming it records a thread.renamed timeline event.
    display_name        TEXT        DEFAULT NULL,
    CONSTRAINT workspace_threads_display_name_length CHECK (display_name IS NULL OR length(trim(display_name)) BETWEEN 1 AND 255),

    -- Lifecycle state (open/closed). `closed_at IS NULL` means open; a timestamp
    -- means closed, and `closed_by` records who closed it (kept for the audit
    -- trail; SET NULL if that account is removed). This is the current state; the
    -- per-transition history lives in workspace_thread_events.
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
    CONSTRAINT workspace_threads_closed_after_created CHECK (closed_at IS NULL OR closed_at >= created_at),

    -- When a file is set, it is referenced with its workspace against
    -- workspace_files (workspace_id, id), so the denormalized workspace_id must
    -- match the file's own — a thread on a file from another workspace cannot be
    -- stored, and removing the file cascades its threads away. With the default
    -- MATCH SIMPLE, a NULL file_id skips this check, so a workspace-level thread
    -- (no file) is allowed.
    CONSTRAINT workspace_threads_file_fkey FOREIGN KEY (workspace_id, file_id)
        REFERENCES workspace_files (workspace_id, id) ON DELETE CASCADE
);

-- Thread anchors: locations within a thread's file the thread is pinned to. A
-- thread may have several, added and removed over its lifetime; removal is a soft
-- delete so the timeline's anchor.removed event keeps its referent.
CREATE TABLE workspace_thread_anchors (
    -- Primary identifier
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References. The thread this anchor pins; deleting the thread removes it.
    thread_id           UUID        NOT NULL REFERENCES workspace_threads (id) ON DELETE CASCADE,

    -- The location, as a modality-tagged anchor (page region, time range, text
    -- span, or table cell). Stored as the anchor's typed JSON.
    anchor              JSONB       NOT NULL,
    CONSTRAINT workspace_thread_anchors_size CHECK (length(anchor::TEXT) <= 8192),

    -- Lifecycle timestamps. Removal is a soft delete (`deleted_at` set).
    created_at          TIMESTAMPTZ NOT NULL DEFAULT current_timestamp,
    deleted_at          TIMESTAMPTZ DEFAULT NULL,
    CONSTRAINT workspace_thread_anchors_deleted_after_created CHECK (deleted_at IS NULL OR deleted_at >= created_at)
);

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

-- Thread timeline events: the non-message entries in a thread's stream (opened,
-- closed, reopened, anchor added/removed). Immutable — an event is a fact that
-- happened, so there is no update or soft-delete; deleting the thread removes them.
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

    -- Event-specific detail, when any: for an anchor event, a snapshot of the
    -- anchor (its id and the anchor JSON), so the timeline renders it without the
    -- anchor row (which may since have been removed). NULL for open/close/reopen.
    target              JSONB       DEFAULT NULL,
    CONSTRAINT workspace_thread_events_target_size CHECK (target IS NULL OR length(target::TEXT) <= 8192),

    -- When it happened (events are immutable, so only a creation timestamp).
    created_at          TIMESTAMPTZ NOT NULL DEFAULT current_timestamp
);

-- A file's threads, newest first (the thread list on a document); only
-- file-pinned threads, so workspace-level threads do not bloat the index.
CREATE INDEX workspace_threads_file_idx
    ON workspace_threads (file_id, created_at DESC)
    WHERE file_id IS NOT NULL AND deleted_at IS NULL;

-- Workspace-scoped thread listing, newest first, filterable by open/closed.
CREATE INDEX workspace_threads_workspace_idx
    ON workspace_threads (workspace_id, created_at DESC)
    WHERE deleted_at IS NULL;

-- A thread's live anchors, oldest first.
CREATE INDEX workspace_thread_anchors_thread_idx
    ON workspace_thread_anchors (thread_id, created_at)
    WHERE deleted_at IS NULL;

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

-- A thread's timeline events, oldest first (merged with comments by the reader).
CREATE INDEX workspace_thread_events_thread_idx
    ON workspace_thread_events (thread_id, created_at);

-- Auto-maintain updated_at on writes (the tables that have the column).
SELECT setup_updated_at('workspace_threads');
SELECT setup_updated_at('workspace_thread_comments');

COMMENT ON TABLE workspace_threads IS 'A closable, optionally file-anchored discussion thread on a file.';
COMMENT ON COLUMN workspace_threads.id IS 'Unique thread identifier';
COMMENT ON COLUMN workspace_threads.workspace_id IS 'Denormalized workspace scope for fast per-workspace thread queries';
COMMENT ON COLUMN workspace_threads.file_id IS 'File the thread is pinned to; NULL for a workspace-level thread';
COMMENT ON COLUMN workspace_threads.author_account_id IS 'Account that opened the thread';
COMMENT ON COLUMN workspace_threads.display_name IS 'Optional human-readable title; NULL for an untitled thread (1-255 chars)';
COMMENT ON COLUMN workspace_threads.closed_at IS 'When the thread was closed; NULL means open';
COMMENT ON COLUMN workspace_threads.closed_by IS 'Account that closed the thread; null if open or that account was removed';
COMMENT ON COLUMN workspace_threads.created_at IS 'Timestamp when the thread was opened';
COMMENT ON COLUMN workspace_threads.updated_at IS 'Timestamp of the last update';
COMMENT ON COLUMN workspace_threads.deleted_at IS 'Soft-deletion timestamp; NULL means live';

COMMENT ON TABLE workspace_thread_anchors IS 'A location within a thread''s file the thread is pinned to; a thread may have several.';
COMMENT ON COLUMN workspace_thread_anchors.id IS 'Unique anchor identifier';
COMMENT ON COLUMN workspace_thread_anchors.thread_id IS 'Thread this anchor pins';
COMMENT ON COLUMN workspace_thread_anchors.anchor IS 'Modality-tagged location (page region, time range, text span, table cell), as typed JSON';
COMMENT ON COLUMN workspace_thread_anchors.created_at IS 'Timestamp when the anchor was added';
COMMENT ON COLUMN workspace_thread_anchors.deleted_at IS 'Soft-removal timestamp; NULL means live';

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

COMMENT ON TABLE workspace_thread_events IS 'An immutable non-message entry in a thread timeline: opened, closed, reopened, renamed, or anchor added/removed.';
COMMENT ON COLUMN workspace_thread_events.id IS 'Unique event identifier';
COMMENT ON COLUMN workspace_thread_events.workspace_id IS 'Denormalized workspace scope';
COMMENT ON COLUMN workspace_thread_events.thread_id IS 'Thread this event belongs to';
COMMENT ON COLUMN workspace_thread_events.kind IS 'What happened (thread.opened, thread.closed, thread.reopened, thread.renamed, thread.anchor.added, thread.anchor.removed)';
COMMENT ON COLUMN workspace_thread_events.actor_account_id IS 'Account that performed the action; null if that account was removed';
COMMENT ON COLUMN workspace_thread_events.target IS 'Event-specific detail (an anchor snapshot for anchor events, the new name for a rename); NULL for open/close/reopen';
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
ALTER TYPE ACTIVITY_TYPE ADD VALUE IF NOT EXISTS 'thread.anchor.added';
ALTER TYPE ACTIVITY_TYPE ADD VALUE IF NOT EXISTS 'thread.anchor.removed';
ALTER TYPE ACTIVITY_TYPE ADD VALUE IF NOT EXISTS 'thread.comment.created';

-- Webhooks carry the thread lifecycle (message-level noise is left off).
ALTER TYPE WEBHOOK_EVENT ADD VALUE IF NOT EXISTS 'thread.opened';
ALTER TYPE WEBHOOK_EVENT ADD VALUE IF NOT EXISTS 'thread.closed';
ALTER TYPE WEBHOOK_EVENT ADD VALUE IF NOT EXISTS 'thread.reopened';
ALTER TYPE WEBHOOK_EVENT ADD VALUE IF NOT EXISTS 'thread.renamed';
ALTER TYPE WEBHOOK_EVENT ADD VALUE IF NOT EXISTS 'thread.anchor.added';
ALTER TYPE WEBHOOK_EVENT ADD VALUE IF NOT EXISTS 'thread.anchor.removed';

-- In-app notifications go to each mentioned account.
ALTER TYPE NOTIFICATION_EVENT ADD VALUE IF NOT EXISTS 'comment.mentioned';
