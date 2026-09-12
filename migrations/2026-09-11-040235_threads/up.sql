-- Threads: threaded discussion modeled after GitHub issues. Two kinds share the
-- table. A workspace thread is a free discussion with an open/closed lifecycle.
-- A document thread IS the review of its document (exactly one per document): it
-- carries an assignee and a derived review_status; its stream interleaves comments
-- review events (detection/redaction created, verified, reopened, assigned).
-- Transitions are recorded both as in-thread timeline events and as workspace
-- events (activity log + webhooks).

-- Kind of a thread timeline event (a non-message entry in a thread's stream).
-- A file thread is the review of its file: alongside the discussion lifecycle
-- (opened/closed/reopened/renamed) it records the review's own transitions —
-- a detection ran, a redaction was made, the review was verified — and the
-- assignee changing. Workspace threads use only the discussion lifecycle.
CREATE TYPE THREAD_EVENT_KIND AS ENUM (
    'thread.opened',            -- The thread was opened
    'thread.closed',            -- The thread was closed (workspace threads)
    'thread.reopened',          -- The thread was reopened (workspace threads)
    'thread.renamed',           -- The thread's display name was changed
    'review.detection.created', -- A detection ran on the document (review needed)
    'review.redaction.created', -- A redaction (review pass) was made
    'review.verified',          -- The review was approved
    'review.reopened',          -- A new detection reopened a verified review
    'review.assigned',          -- The review was assigned to a reviewer
    'review.unassigned'         -- The review's assignee was cleared
);

COMMENT ON TYPE THREAD_EVENT_KIND IS 'The kind of a non-message entry in a thread timeline: the discussion lifecycle (opened/closed/reopened/renamed) and, for a file thread, its review transitions (detection/redaction created, verified, reopened, assigned/unassigned).';

-- The review state of a file thread. NULL for a workspace thread (no review).
-- Derived from review events, never set by hand: a detection makes it
-- `needs_review`, a redaction `in_review`, verification `resolved`; a later
-- detection reopens it to `needs_review`.
CREATE TYPE REVIEW_STATUS AS ENUM (
    'needs_review', -- A detection exists; no redaction has been reviewed yet
    'in_review',    -- A redaction (review pass) exists but is not verified
    'resolved'      -- The review has been verified
);

COMMENT ON TYPE REVIEW_STATUS IS 'The review state of a file thread, derived from review events: needs_review, in_review, or resolved.';

-- Threads: a workspace discussion or a document review, both closable/soft-deletable.
CREATE TABLE workspace_threads (
    -- Primary identifier
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References. A thread belongs to a workspace and is optionally about one of
    -- its documents: `document_id` NULL is a workspace-level discussion; a set `document_id`
    -- makes it that document's review thread (exactly one per document).
    workspace_id        UUID        NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    document_id             UUID        DEFAULT NULL,

    -- The account that opened the thread. If it is removed, the thread goes too.
    author_account_id   UUID        NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,

    -- Optional human-readable title (like a GitHub issue title); NULL for an
    -- untitled thread. Renaming it records a thread.renamed timeline event.
    display_name        TEXT        DEFAULT NULL,
    CONSTRAINT workspace_threads_display_name_length CHECK (display_name IS NULL OR length(trim(display_name)) BETWEEN 1 AND 255),

    -- Review facets (a document thread is the review of its document). The reviewer who
    -- owns the review; NULL when unassigned (a review can be in progress with no
    -- assignee). SET NULL if that account is removed.
    assignee_account_id UUID        DEFAULT NULL REFERENCES accounts (id) ON DELETE SET NULL,

    -- The review state, present only for a file thread and NULL for a
    -- workspace-level one. Derived from review events (detection/redaction/verify),
    -- never set by a user. The `(document_id IS NULL) = (review_status IS NULL)` check
    -- keeps the two consistent: exactly the document threads carry a review status.
    review_status       REVIEW_STATUS DEFAULT NULL,
    CONSTRAINT workspace_threads_review_status_file CHECK (
        (document_id IS NULL) = (review_status IS NULL)
    ),

    -- Lifecycle state for a workspace thread (open/closed). `closed_at IS NULL`
    -- means open; a timestamp means closed, and `closed_by` records who closed it
    -- (SET NULL if that account is removed). A file thread uses `review_status`
    -- instead and is never closed this way. This is the current state; the
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

    -- When a document is set, it is referenced with its workspace against
    -- workspace_documents (workspace_id, id), so the denormalized workspace_id must
    -- match the document's own — a thread on a document from another workspace cannot be
    -- stored, and removing the document cascades its threads away. With the default
    -- MATCH SIMPLE, a NULL document_id skips this check, so a workspace-level thread
    -- (no file) is allowed.
    CONSTRAINT workspace_threads_document_fkey FOREIGN KEY (workspace_id, document_id)
        REFERENCES workspace_documents (workspace_id, id) ON DELETE CASCADE
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

-- Thread timeline events: the non-message entries in a thread's stream (the
-- discussion lifecycle and, for a file thread, its review transitions). Immutable
-- — an event is a fact that happened, so there is no update or soft-delete;
-- deleting the thread removes them.
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

    -- Event-specific detail, when any: the new name for a rename, the assignee for
    -- an assign, or the detection/redaction id for a review event. NULL otherwise.
    target              JSONB       DEFAULT NULL,
    CONSTRAINT workspace_thread_events_target_size CHECK (target IS NULL OR length(target::TEXT) <= 8192),

    -- When it happened (events are immutable, so only a creation timestamp).
    created_at          TIMESTAMPTZ NOT NULL DEFAULT current_timestamp
);

-- A document has exactly one live review thread: the document thread IS the
-- review of that document. This partial-unique index enforces the one-per-document
-- rule and backs the find-or-create lookup; workspace-level threads (NULL
-- document_id) are exempt.
CREATE UNIQUE INDEX workspace_threads_document_idx
    ON workspace_threads (document_id)
    WHERE document_id IS NOT NULL AND deleted_at IS NULL;

-- Workspace-scoped thread listing, newest first, filterable by open/closed.
CREATE INDEX workspace_threads_workspace_idx
    ON workspace_threads (workspace_id, created_at DESC)
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

COMMENT ON TABLE workspace_threads IS 'A discussion thread: a workspace thread (free discussion) or a file thread (the review of its file, carrying an assignee and derived review_status).';
COMMENT ON COLUMN workspace_threads.id IS 'Unique thread identifier';
COMMENT ON COLUMN workspace_threads.workspace_id IS 'Denormalized workspace scope for fast per-workspace thread queries';
COMMENT ON COLUMN workspace_threads.document_id IS 'Document the thread reviews; NULL for a workspace-level thread';
COMMENT ON COLUMN workspace_threads.author_account_id IS 'Account that opened the thread';
COMMENT ON COLUMN workspace_threads.display_name IS 'Optional human-readable title; NULL for an untitled thread (1-255 chars)';
COMMENT ON COLUMN workspace_threads.closed_at IS 'When the thread was closed; NULL means open';
COMMENT ON COLUMN workspace_threads.closed_by IS 'Account that closed the thread; null if open or that account was removed';
COMMENT ON COLUMN workspace_threads.created_at IS 'Timestamp when the thread was opened';
COMMENT ON COLUMN workspace_threads.updated_at IS 'Timestamp of the last update';
COMMENT ON COLUMN workspace_threads.deleted_at IS 'Soft-deletion timestamp; NULL means live';

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

COMMENT ON TABLE workspace_thread_events IS 'An immutable non-message entry in a thread timeline: the discussion lifecycle (opened/closed/reopened/renamed) and a file thread''s review transitions (detection/redaction created, verified, reopened, assigned/unassigned).';
COMMENT ON COLUMN workspace_thread_events.id IS 'Unique event identifier';
COMMENT ON COLUMN workspace_thread_events.workspace_id IS 'Denormalized workspace scope';
COMMENT ON COLUMN workspace_thread_events.thread_id IS 'Thread this event belongs to';
COMMENT ON COLUMN workspace_thread_events.kind IS 'What happened (thread.opened/closed/reopened/renamed; review.detection.created/redaction.created/verified/reopened/assigned/unassigned)';
COMMENT ON COLUMN workspace_thread_events.actor_account_id IS 'Account that performed the action; null if that account was removed';
COMMENT ON COLUMN workspace_thread_events.target IS 'Event-specific detail (the new name for a rename, the assignee for assign, the detection/redaction id for a review event); NULL when none';
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
-- A file thread's review lifecycle. Detection/redaction creation is already
-- logged by the detection subsystem, so the activity log adds only the review's
-- own gestures: verification and (re)assignment.
ALTER TYPE ACTIVITY_TYPE ADD VALUE IF NOT EXISTS 'review.verified';
ALTER TYPE ACTIVITY_TYPE ADD VALUE IF NOT EXISTS 'review.assigned';
ALTER TYPE ACTIVITY_TYPE ADD VALUE IF NOT EXISTS 'review.unassigned';

-- Webhooks carry the thread lifecycle (message-level noise is left off) plus the
-- review's verification and assignment gestures.
ALTER TYPE WEBHOOK_EVENT ADD VALUE IF NOT EXISTS 'thread.opened';
ALTER TYPE WEBHOOK_EVENT ADD VALUE IF NOT EXISTS 'thread.closed';
ALTER TYPE WEBHOOK_EVENT ADD VALUE IF NOT EXISTS 'thread.reopened';
ALTER TYPE WEBHOOK_EVENT ADD VALUE IF NOT EXISTS 'thread.renamed';
ALTER TYPE WEBHOOK_EVENT ADD VALUE IF NOT EXISTS 'review.verified';
ALTER TYPE WEBHOOK_EVENT ADD VALUE IF NOT EXISTS 'review.assigned';
ALTER TYPE WEBHOOK_EVENT ADD VALUE IF NOT EXISTS 'review.unassigned';

-- In-app notifications go to each mentioned account, and to a reviewer when a
-- review is assigned to them.
ALTER TYPE NOTIFICATION_EVENT ADD VALUE IF NOT EXISTS 'comment.mentioned';
ALTER TYPE NOTIFICATION_EVENT ADD VALUE IF NOT EXISTS 'review.assigned';
