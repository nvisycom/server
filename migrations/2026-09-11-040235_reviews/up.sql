-- Document reviews: a discussion on a document, with a manual sign-off workflow.
--
-- A review is the unit of collaboration on a document: a named discussion (an
-- author, a title, a stream of comments and timeline events) with a sign-off
-- lifecycle (needs_review → in_review → resolved) layered on top. A document has
-- 0..N reviews — none when someone works alone, one or more when a formal sign-off
-- is wanted, each for a distinct audience. A review references — does not own — the
-- detections and redactions done for it, via the link tables here, and its activity
-- (comments, status changes, links, assignment, verification) is one timeline in
-- workspace_review_events.
--
-- This subsystem sits on top of documents, detections, and redactions (all
-- introduced by earlier migrations).

-- The state of a review. A manual reviewer workflow: a review opens at
-- `needs_review`, moves to `in_review` when a reviewer takes it (the first
-- assignee), and `resolved` when verified. Reopening or removing the last assignee
-- returns it to `needs_review`.
CREATE TYPE REVIEW_STATUS AS ENUM (
    'needs_review', -- Opened, awaiting a reviewer
    'in_review',    -- A reviewer has taken it (assigned/started)
    'resolved'      -- Verified
);

COMMENT ON TYPE REVIEW_STATUS IS 'The state of a review: needs_review, in_review, or resolved.';

-- Reviews: a named discussion on a document with a sign-off lifecycle. A document
-- has 0..N reviews (each for a distinct audience). A review carries its state and
-- references (does not own) its assignees and the detections and redactions done
-- for it via the link tables below; its comments and timeline events belong to it.
CREATE TABLE workspace_reviews (
    -- Primary identifier
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References. Denormalized workspace scope and the document under review. The
    -- document is referenced with its workspace against workspace_documents
    -- (workspace_id, id), so the denormalized workspace_id must match the
    -- document's own; removing the document cascades the review away.
    workspace_id        UUID        NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    document_id         UUID        NOT NULL,

    -- The account that opened the review. If it is removed, the review goes too.
    author_account_id   UUID        NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,

    -- Human-readable title (like a GitHub issue title). Required. Renaming it
    -- records a review.renamed timeline event.
    display_name        TEXT        NOT NULL,
    CONSTRAINT workspace_reviews_display_name_length CHECK (length(trim(display_name)) BETWEEN 1 AND 255),

    -- The review state, and the whole lifecycle: a review opens at `needs_review`,
    -- moves to `in_review` when a reviewer takes it, and `resolved` when verified
    -- (resolved is "done" — there is no separate open/closed discussion flag).
    -- Assignees live in the workspace_review_assignees link table (0..N reviewers).
    review_status       REVIEW_STATUS NOT NULL DEFAULT 'needs_review',

    -- Lifecycle timestamps
    created_at          TIMESTAMPTZ NOT NULL DEFAULT current_timestamp,
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT current_timestamp,
    deleted_at          TIMESTAMPTZ DEFAULT NULL,
    CONSTRAINT workspace_reviews_updated_after_created CHECK (updated_at >= created_at),
    CONSTRAINT workspace_reviews_deleted_after_created CHECK (deleted_at IS NULL OR deleted_at >= created_at),

    -- A document may have many reviews, so document_id is deliberately not unique.
    CONSTRAINT workspace_reviews_document_fkey FOREIGN KEY (workspace_id, document_id)
        REFERENCES workspace_documents (workspace_id, id) ON DELETE CASCADE
);

-- A document's reviews, newest first (backs the per-document review listing).
CREATE INDEX workspace_reviews_document_idx
    ON workspace_reviews (document_id, created_at DESC)
    WHERE deleted_at IS NULL;

-- The review queue: a workspace's reviews by status, newest first (backs the
-- status-filtered listing and the "needs review" queue).
CREATE INDEX workspace_reviews_workspace_idx
    ON workspace_reviews (workspace_id, review_status, created_at DESC)
    WHERE deleted_at IS NULL;

-- Keep updated_at current on every row modification.
SELECT setup_updated_at('workspace_reviews');

COMMENT ON TABLE workspace_reviews IS 'A named discussion on a document with a manual sign-off lifecycle (0..N per document). References the detections/redactions done for it via link tables; owns its comments and timeline.';
COMMENT ON COLUMN workspace_reviews.id IS 'Unique review identifier';
COMMENT ON COLUMN workspace_reviews.workspace_id IS 'Denormalized workspace scope';
COMMENT ON COLUMN workspace_reviews.document_id IS 'Document under review (a document may have many reviews)';
COMMENT ON COLUMN workspace_reviews.author_account_id IS 'Account that opened the review';
COMMENT ON COLUMN workspace_reviews.display_name IS 'Human-readable title (1-255 chars)';
COMMENT ON COLUMN workspace_reviews.review_status IS 'Review state and lifecycle (needs_review/in_review/resolved)';
COMMENT ON COLUMN workspace_reviews.created_at IS 'Timestamp when the review was opened';
COMMENT ON COLUMN workspace_reviews.updated_at IS 'Timestamp of the last update';
COMMENT ON COLUMN workspace_reviews.deleted_at IS 'Soft-deletion timestamp; NULL means live';

-- Review comments: one message within a review's discussion.
CREATE TABLE workspace_review_comments (
    -- Primary identifier
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- The comment this one answers, when it is a reply: for an assistant reply,
    -- the message that addressed the assistant; NULL for an ordinary message. A
    -- unique index below allows at most one live reply per parent, so a
    -- redelivered assistant job cannot post a second reply.
    parent_id           UUID        DEFAULT NULL REFERENCES workspace_review_comments (id) ON DELETE SET NULL,

    -- References. Denormalized workspace scope for fast per-workspace queries, and
    -- the review this message belongs to; deleting the review removes its comments
    -- (CASCADE), so deleting a review takes its messages.
    workspace_id        UUID        NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    review_id           UUID        NOT NULL REFERENCES workspace_reviews (id) ON DELETE CASCADE,

    -- The message author. If their account is removed, their comments go with it.
    author_account_id   UUID        NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,

    -- The message text.
    body                TEXT        NOT NULL,
    CONSTRAINT workspace_review_comments_body_length CHECK (length(trim(body)) BETWEEN 1 AND 10000),

    -- Lifecycle timestamps
    created_at          TIMESTAMPTZ NOT NULL DEFAULT current_timestamp,
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT current_timestamp,
    deleted_at          TIMESTAMPTZ DEFAULT NULL,
    CONSTRAINT workspace_review_comments_updated_after_created CHECK (updated_at >= created_at),
    CONSTRAINT workspace_review_comments_deleted_after_created CHECK (deleted_at IS NULL OR deleted_at >= created_at)
);

-- A review's messages, oldest first (a discussion reads top to bottom).
CREATE INDEX workspace_review_comments_review_idx
    ON workspace_review_comments (review_id, created_at)
    WHERE deleted_at IS NULL;

-- At most one live reply per parent comment: a redelivered assistant job
-- re-inserting its reply hits this and is treated as already answered, so the
-- assistant never double-replies to one mention.
CREATE UNIQUE INDEX workspace_review_comments_parent_unique_idx
    ON workspace_review_comments (parent_id)
    WHERE parent_id IS NOT NULL AND deleted_at IS NULL;

-- Keep updated_at current on every row modification.
SELECT setup_updated_at('workspace_review_comments');

COMMENT ON TABLE workspace_review_comments IS 'One message within a review''s discussion.';
COMMENT ON COLUMN workspace_review_comments.id IS 'Unique comment identifier';
COMMENT ON COLUMN workspace_review_comments.parent_id IS 'For a reply, the comment it answers; NULL otherwise (one live reply per parent)';
COMMENT ON COLUMN workspace_review_comments.workspace_id IS 'Denormalized workspace scope';
COMMENT ON COLUMN workspace_review_comments.review_id IS 'Review this message belongs to';
COMMENT ON COLUMN workspace_review_comments.author_account_id IS 'Account that wrote the message';
COMMENT ON COLUMN workspace_review_comments.body IS 'Message text (1-10000 chars)';
COMMENT ON COLUMN workspace_review_comments.created_at IS 'Timestamp when the comment was posted';
COMMENT ON COLUMN workspace_review_comments.updated_at IS 'Timestamp of the last edit';
COMMENT ON COLUMN workspace_review_comments.deleted_at IS 'Soft-deletion timestamp; NULL means live';

-- Review assignees: the reviewers responsible for a review (0..N). A review with
-- no assignees is unassigned; the first assignee moves it to `in_review` and
-- removing the last returns it to `needs_review` (handled in the query layer).
CREATE TABLE workspace_review_assignees (
    review_id  UUID NOT NULL REFERENCES workspace_reviews (id) ON DELETE CASCADE,
    account_id UUID NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT current_timestamp,
    PRIMARY KEY (review_id, account_id)
);

-- A reviewer's assigned reviews (backs the "my reviews" queue).
CREATE INDEX workspace_review_assignees_account_idx
    ON workspace_review_assignees (account_id);

COMMENT ON TABLE workspace_review_assignees IS 'The reviewers assigned to a review (many-to-many); a review may have 0..N assignees.';
COMMENT ON COLUMN workspace_review_assignees.review_id IS 'The review';
COMMENT ON COLUMN workspace_review_assignees.account_id IS 'The assigned reviewer';
COMMENT ON COLUMN workspace_review_assignees.created_at IS 'When the reviewer was assigned';

-- Review ← detection links: the detections a review references. Many-to-many — a
-- shared detection's findings can feed redactions across several reviews, so a
-- detection may be referenced by more than one review.
CREATE TABLE workspace_review_detections (
    review_id    UUID NOT NULL REFERENCES workspace_reviews (id) ON DELETE CASCADE,
    detection_id UUID NOT NULL REFERENCES workspace_detections (id) ON DELETE CASCADE,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT current_timestamp,
    PRIMARY KEY (review_id, detection_id)
);

-- Reverse lookup: which reviews reference a given detection.
CREATE INDEX workspace_review_detections_detection_idx
    ON workspace_review_detections (detection_id);

COMMENT ON TABLE workspace_review_detections IS 'Links a review to the detections it references (many-to-many).';
COMMENT ON COLUMN workspace_review_detections.review_id IS 'The referencing review';
COMMENT ON COLUMN workspace_review_detections.detection_id IS 'The referenced detection';
COMMENT ON COLUMN workspace_review_detections.created_at IS 'When the link was made';

-- Review ← redaction links: the redaction passes a review references. Many-to-many
-- for the same reason as detections.
CREATE TABLE workspace_review_redactions (
    review_id    UUID NOT NULL REFERENCES workspace_reviews (id) ON DELETE CASCADE,
    redaction_id UUID NOT NULL REFERENCES workspace_redactions (id) ON DELETE CASCADE,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT current_timestamp,
    PRIMARY KEY (review_id, redaction_id)
);

-- Reverse lookup: which reviews reference a given redaction.
CREATE INDEX workspace_review_redactions_redaction_idx
    ON workspace_review_redactions (redaction_id);

COMMENT ON TABLE workspace_review_redactions IS 'Links a review to the redactions it references (many-to-many).';
COMMENT ON COLUMN workspace_review_redactions.review_id IS 'The referencing review';
COMMENT ON COLUMN workspace_review_redactions.redaction_id IS 'The referenced redaction';
COMMENT ON COLUMN workspace_review_redactions.created_at IS 'When the link was made';

-- Kind of a review timeline event (a non-message entry in a review's stream): the
-- lifecycle (opened, renamed, verified/resolved, reopened) and the sign-off
-- workflow (detections/redactions linked, assignment changes). Comments are the
-- message entries; these are everything else in the one timeline.
CREATE TYPE REVIEW_EVENT_KIND AS ENUM (
    'review.opened',    -- The review was opened
    'review.renamed',   -- The review's display name was changed
    'detection.linked', -- A detection was referenced by the review
    'redaction.linked', -- A redaction was referenced by the review
    'assigned',         -- The review was assigned to a reviewer
    'unassigned',       -- A reviewer was unassigned
    'verified',         -- The review was verified (resolved)
    'reopened'          -- A resolved review was reopened (back to needs_review)
);

COMMENT ON TYPE REVIEW_EVENT_KIND IS 'The kind of a non-message entry in a review timeline: the lifecycle (opened, renamed, verified, reopened) and the sign-off workflow (detection/redaction linked, assigned/unassigned).';

-- Review timeline events: the non-message entries in a review's stream. Immutable —
-- an event is a fact that happened, so there is no update or soft-delete; deleting
-- the review removes them. The reader merges these with the review's comments into
-- one timeline.
CREATE TABLE workspace_review_events (
    -- Primary identifier
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References. Denormalized workspace scope (matching the sibling tables) and
    -- the review this event belongs to.
    workspace_id        UUID        NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    review_id           UUID        NOT NULL REFERENCES workspace_reviews (id) ON DELETE CASCADE,

    -- What happened.
    kind                REVIEW_EVENT_KIND NOT NULL,

    -- Who did it. If their account is removed, keep the event but forget the actor
    -- (the transition still happened).
    actor_account_id    UUID        DEFAULT NULL REFERENCES accounts (id) ON DELETE SET NULL,

    -- Event-specific detail, when any: the new name for a rename, the linked
    -- detection/redaction id, or the assignee for an assign. NULL otherwise.
    target              JSONB       DEFAULT NULL,
    CONSTRAINT workspace_review_events_target_size CHECK (target IS NULL OR length(target::TEXT) <= 8192),

    -- When it happened (events are immutable, so only a creation timestamp).
    created_at          TIMESTAMPTZ NOT NULL DEFAULT current_timestamp
);

-- A review's timeline events, oldest first (merged with comments by the reader).
CREATE INDEX workspace_review_events_review_idx
    ON workspace_review_events (review_id, created_at);

COMMENT ON TABLE workspace_review_events IS 'An immutable non-message entry in a review timeline: the discussion lifecycle (opened/closed/reopened/renamed) and the sign-off workflow (detection/redaction linked, assigned/unassigned, verified).';
COMMENT ON COLUMN workspace_review_events.id IS 'Unique event identifier';
COMMENT ON COLUMN workspace_review_events.workspace_id IS 'Denormalized workspace scope';
COMMENT ON COLUMN workspace_review_events.review_id IS 'Review this event belongs to';
COMMENT ON COLUMN workspace_review_events.kind IS 'What happened (review.opened/closed/reopened/renamed, detection/redaction.linked, assigned/unassigned, verified)';
COMMENT ON COLUMN workspace_review_events.actor_account_id IS 'Account that performed the action; null if that account was removed';
COMMENT ON COLUMN workspace_review_events.target IS 'Event-specific detail (the new name, a linked id, an assignee); NULL when none';
COMMENT ON COLUMN workspace_review_events.created_at IS 'Timestamp when the event happened';

-- Review lifecycle events feed the event sinks; each value is added by this
-- migration (the one that introduces reviews). ALTER TYPE only adds labels here
-- (no rows use them yet), so it stays transactional.
--
-- Activity log records the review lifecycle plus each message posted.
ALTER TYPE ACTIVITY_TYPE ADD VALUE IF NOT EXISTS 'review.opened';
ALTER TYPE ACTIVITY_TYPE ADD VALUE IF NOT EXISTS 'review.renamed';
ALTER TYPE ACTIVITY_TYPE ADD VALUE IF NOT EXISTS 'review.deleted';
ALTER TYPE ACTIVITY_TYPE ADD VALUE IF NOT EXISTS 'review.comment.created';
ALTER TYPE ACTIVITY_TYPE ADD VALUE IF NOT EXISTS 'review.verified';
ALTER TYPE ACTIVITY_TYPE ADD VALUE IF NOT EXISTS 'review.reopened';
ALTER TYPE ACTIVITY_TYPE ADD VALUE IF NOT EXISTS 'review.assigned';
ALTER TYPE ACTIVITY_TYPE ADD VALUE IF NOT EXISTS 'review.unassigned';

-- Webhooks carry the review lifecycle (message-level noise is left off).
ALTER TYPE WEBHOOK_EVENT ADD VALUE IF NOT EXISTS 'review.opened';
ALTER TYPE WEBHOOK_EVENT ADD VALUE IF NOT EXISTS 'review.renamed';
ALTER TYPE WEBHOOK_EVENT ADD VALUE IF NOT EXISTS 'review.verified';
ALTER TYPE WEBHOOK_EVENT ADD VALUE IF NOT EXISTS 'review.reopened';
ALTER TYPE WEBHOOK_EVENT ADD VALUE IF NOT EXISTS 'review.assigned';
ALTER TYPE WEBHOOK_EVENT ADD VALUE IF NOT EXISTS 'review.unassigned';

-- In-app notifications go to each mentioned account and to a newly-assigned reviewer.
ALTER TYPE NOTIFICATION_EVENT ADD VALUE IF NOT EXISTS 'comment.mentioned';
ALTER TYPE NOTIFICATION_EVENT ADD VALUE IF NOT EXISTS 'review.assigned';
