-- Comments: threaded discussion on a file under review. A comment is authored by
-- a workspace member, optionally anchored to a location within the file (a page
-- region, a time range, a text span — the modality-tagged anchor), optionally a
-- reply to another comment (one level), and can be resolved to close a thread.

-- Comments table: one comment on a file.
CREATE TABLE workspace_comments (
    -- Primary identifier
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References. The workspace is denormalized onto the row (rather than reached
    -- through the file) so the common "comments across the workspace" query is a
    -- single indexed scan with no join to workspace_files.
    workspace_id        UUID        NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    file_id             UUID        NOT NULL,

    -- The comment's author. If their account is removed, their comments go with it.
    author_account_id   UUID        NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,

    -- A reply's parent, for one-level threads. NULL for a top-level comment. A
    -- reply is removed with its parent (CASCADE), so a resolved/deleted thread
    -- takes its replies. A parent is itself always top-level (enforced in the
    -- repository), so threads never nest deeper than one level.
    parent_id           UUID        DEFAULT NULL REFERENCES workspace_comments (id) ON DELETE CASCADE,

    -- The comment text.
    body                TEXT        NOT NULL,
    CONSTRAINT workspace_comments_body_length CHECK (length(trim(body)) BETWEEN 1 AND 10000),

    -- Optional location within the file the comment is pinned to, as a
    -- modality-tagged anchor (page region, time range, text span, or table cell).
    -- NULL for a file-level comment with no pin. Stored as the anchor's typed JSON.
    anchor              JSONB       DEFAULT NULL,
    CONSTRAINT workspace_comments_anchor_size CHECK (anchor IS NULL OR length(anchor::TEXT) <= 8192),

    -- Resolution: a resolved thread is closed. `resolved_at IS NULL` means open;
    -- a timestamp means resolved, and `resolved_by` records who resolved it (kept
    -- for the audit trail; SET NULL if that account is removed). Only a top-level
    -- comment is resolvable (a reply inherits its thread's state).
    resolved_at         TIMESTAMPTZ DEFAULT NULL,
    resolved_by         UUID        DEFAULT NULL REFERENCES accounts (id) ON DELETE SET NULL,
    CONSTRAINT workspace_comments_resolved_consistent CHECK (
        (resolved_at IS NULL) = (resolved_by IS NULL)
    ),

    -- Lifecycle timestamps
    created_at          TIMESTAMPTZ NOT NULL DEFAULT current_timestamp,
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT current_timestamp,
    deleted_at          TIMESTAMPTZ DEFAULT NULL,
    CONSTRAINT workspace_comments_updated_after_created CHECK (updated_at >= created_at),
    CONSTRAINT workspace_comments_deleted_after_created CHECK (deleted_at IS NULL OR deleted_at >= created_at),
    CONSTRAINT workspace_comments_resolved_after_created CHECK (resolved_at IS NULL OR resolved_at >= created_at),

    -- The file is referenced with its workspace, against
    -- workspace_files (workspace_id, id), so the denormalized workspace_id must
    -- match the file's own — a comment on a file from another workspace cannot be
    -- stored. Removing the file cascades its comments away.
    CONSTRAINT workspace_comments_file_fkey FOREIGN KEY (workspace_id, file_id)
        REFERENCES workspace_files (workspace_id, id) ON DELETE CASCADE
);

-- A file's comment thread, oldest first (a discussion reads top to bottom).
CREATE INDEX workspace_comments_file_idx
    ON workspace_comments (file_id, created_at)
    WHERE deleted_at IS NULL;

-- A thread's replies, oldest first.
CREATE INDEX workspace_comments_parent_idx
    ON workspace_comments (parent_id, created_at)
    WHERE parent_id IS NOT NULL AND deleted_at IS NULL;

-- Workspace-scoped listing, newest first.
CREATE INDEX workspace_comments_workspace_idx
    ON workspace_comments (workspace_id, created_at DESC)
    WHERE deleted_at IS NULL;

-- An author's comments, newest first.
CREATE INDEX workspace_comments_author_idx
    ON workspace_comments (author_account_id, created_at DESC)
    WHERE deleted_at IS NULL;

-- Auto-maintain updated_at on writes (soft-delete column present).
SELECT setup_updated_at('workspace_comments');

COMMENT ON TABLE workspace_comments IS 'Threaded comments on a file under review, optionally anchored to a location.';
COMMENT ON COLUMN workspace_comments.id IS 'Unique comment identifier';
COMMENT ON COLUMN workspace_comments.workspace_id IS 'Denormalized workspace scope for fast per-workspace comment queries';
COMMENT ON COLUMN workspace_comments.file_id IS 'File the comment is on';
COMMENT ON COLUMN workspace_comments.author_account_id IS 'Account that wrote the comment';
COMMENT ON COLUMN workspace_comments.parent_id IS 'Parent comment for a one-level reply; NULL for a top-level comment';
COMMENT ON COLUMN workspace_comments.body IS 'Comment text (1-10000 chars)';
COMMENT ON COLUMN workspace_comments.anchor IS 'Optional modality-tagged location within the file the comment is pinned to; NULL for a file-level comment';
COMMENT ON COLUMN workspace_comments.resolved_at IS 'When the thread was resolved; NULL means open';
COMMENT ON COLUMN workspace_comments.resolved_by IS 'Account that resolved the thread, for the audit trail; null if that account was removed';
COMMENT ON COLUMN workspace_comments.created_at IS 'When the comment was created';
COMMENT ON COLUMN workspace_comments.updated_at IS 'When the comment was last updated';
COMMENT ON COLUMN workspace_comments.deleted_at IS 'Soft-deletion timestamp; NULL means live';

-- Comment lifecycle events feed the event sinks, each value added by this
-- migration (the migration that introduces comments). ALTER TYPE ... ADD VALUE
-- only adds labels here (no rows use them yet), so it stays transactional.
--
-- Activity log records the full lifecycle.
ALTER TYPE ACTIVITY_TYPE ADD VALUE IF NOT EXISTS 'comment.created';
ALTER TYPE ACTIVITY_TYPE ADD VALUE IF NOT EXISTS 'comment.resolved';
ALTER TYPE ACTIVITY_TYPE ADD VALUE IF NOT EXISTS 'comment.deleted';

-- Webhooks carry creation and resolution (deletion is internal).
ALTER TYPE WEBHOOK_EVENT ADD VALUE IF NOT EXISTS 'comment.created';
ALTER TYPE WEBHOOK_EVENT ADD VALUE IF NOT EXISTS 'comment.resolved';

-- In-app notifications go to each mentioned account.
ALTER TYPE NOTIFICATION_EVENT ADD VALUE IF NOT EXISTS 'comment.mentioned';
