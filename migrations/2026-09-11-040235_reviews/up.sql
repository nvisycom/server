-- Document reviews: a discussion on a document, with an explicit approval workflow.
--
-- A review is the unit of collaboration on a document: a named discussion (an
-- author, a title, a stream of comments and timeline events) with a decision-driven
-- approval lifecycle layered on top. A document has 0..N reviews — none when someone
-- works alone, at most one open at a time when a formal sign-off is wanted. A review
-- is anchored to its document, points at the detection it is currently reviewing and
-- at the redaction it approves (direct columns, not link tables), and its activity
-- (comments, status changes, detection/redaction repoints, assignment, approvals) is
-- one timeline in workspace_review_events.
--
-- This subsystem sits on top of documents, detections, and redactions (all
-- introduced by earlier migrations).

-- The state of a review. A decision-driven approval workflow: a review opens at
-- `draft`, moves to `in_review` on submit, and `approved` once enough reviewers
-- approve. A reviewer can request changes (`changes_requested`), the author
-- resubmits, and a partially-approved review (`partially_approved`, reachable only
-- when a workspace requires two or more approvals) awaits the remaining approvals.
-- The state is set by reviewer decisions, not by assignment.
CREATE TYPE REVIEW_STATUS AS ENUM (
    'draft',              -- Opened, edits being assembled; submit sends it for review
    'in_review',          -- Submitted, awaiting a decision
    'changes_requested',  -- A reviewer pushed back; resubmit returns it to in_review
    'partially_approved', -- Some but not all required approvals in (min_approvals >= 2)
    'approved'            -- Signed off; the approved redaction is the output
);

COMMENT ON TYPE REVIEW_STATUS IS 'The state of a review: draft, in_review, changes_requested, partially_approved, or approved.';

-- Reviews: a named discussion on a document with an approval lifecycle. A document
-- has 0..N reviews, which may run concurrently (one per audience). A review is
-- anchored to its document and carries its state, the detection it is currently
-- reviewing, and the redaction it approves as direct columns; its assignees (whose
-- approvals drive sign-off), comments, and timeline events belong to it.
CREATE TABLE workspace_reviews (
    -- Primary identifier
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References. Denormalized workspace scope and the document under review. The
    -- document is referenced with its workspace against workspace_documents
    -- (workspace_id, id), so the denormalized workspace_id must match the
    -- document's own; removing the document cascades the review away.
    workspace_id        UUID        NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    document_id         UUID        NOT NULL,

    -- The detection the review is currently working from, and the redaction it has
    -- approved as its output. Both are nullable: a draft review may have no detection
    -- yet, and redaction_id is set only on approval. Re-detecting the document
    -- repoints detection_id (recorded as a review.redetected event). ON DELETE SET
    -- NULL keeps the review when a referenced detection or redaction is hard-deleted.
    detection_id        UUID        DEFAULT NULL REFERENCES workspace_detections (id) ON DELETE SET NULL,
    redaction_id        UUID        DEFAULT NULL REFERENCES workspace_redactions (id) ON DELETE SET NULL,

    -- The account that opened the review. If it is removed, the review goes too.
    author_account_id   UUID        NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,

    -- Human-readable title (like a GitHub issue title). Required. Renaming it
    -- records a review.renamed timeline event.
    display_name        TEXT        NOT NULL,
    CONSTRAINT workspace_reviews_display_name_length CHECK (length(trim(display_name)) BETWEEN 1 AND 255),

    -- The review state, and the whole lifecycle: a review opens at `draft`, moves to
    -- `in_review` on submit, through `changes_requested` / `partially_approved`, and
    -- `approved` once enough reviewers approve. State is set by explicit reviewer
    -- decisions; approvals are the assignees' stamped `approved_at` values.
    review_status       REVIEW_STATUS NOT NULL DEFAULT 'draft',

    -- Lifecycle timestamps
    created_at          TIMESTAMPTZ NOT NULL DEFAULT current_timestamp,
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT current_timestamp,
    deleted_at          TIMESTAMPTZ DEFAULT NULL,
    CONSTRAINT workspace_reviews_updated_after_created CHECK (updated_at >= created_at),
    CONSTRAINT workspace_reviews_deleted_after_created CHECK (deleted_at IS NULL OR deleted_at >= created_at),

    -- A document may have many concurrent reviews, one per audience.
    CONSTRAINT workspace_reviews_document_fkey FOREIGN KEY (workspace_id, document_id)
        REFERENCES workspace_documents (workspace_id, id) ON DELETE CASCADE
);

-- A document's reviews, newest first (backs the per-document review listing).
CREATE INDEX workspace_reviews_document_idx
    ON workspace_reviews (document_id, created_at DESC)
    WHERE deleted_at IS NULL;

-- The review queue: a workspace's reviews by status, newest first (backs the
-- status-filtered listing and the review queue).
CREATE INDEX workspace_reviews_workspace_idx
    ON workspace_reviews (workspace_id, review_status, created_at DESC)
    WHERE deleted_at IS NULL;

-- Keep updated_at current on every row modification.
SELECT setup_updated_at('workspace_reviews');

COMMENT ON TABLE workspace_reviews IS 'A named discussion on a document with an approval lifecycle (0..N per document, concurrent per audience). Anchored to its document; points at the detection it reviews and the redaction it approves; owns its comments and timeline.';
COMMENT ON COLUMN workspace_reviews.id IS 'Unique review identifier';
COMMENT ON COLUMN workspace_reviews.workspace_id IS 'Denormalized workspace scope';
COMMENT ON COLUMN workspace_reviews.document_id IS 'Document under review (a document may have many concurrent reviews, one per audience)';
COMMENT ON COLUMN workspace_reviews.detection_id IS 'The detection currently under review; NULL before a detection is linked';
COMMENT ON COLUMN workspace_reviews.redaction_id IS 'The approved redaction output; NULL until approved';
COMMENT ON COLUMN workspace_reviews.author_account_id IS 'Account that opened the review';
COMMENT ON COLUMN workspace_reviews.display_name IS 'Human-readable title (1-255 chars)';
COMMENT ON COLUMN workspace_reviews.review_status IS 'Review state and lifecycle (draft/in_review/changes_requested/partially_approved/approved)';
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
    -- redelivered assistant job cannot post a second reply. A reply is deleted
    -- with the parent it answers (CASCADE), so a hard-deleted parent never leaves
    -- an orphan reply promoted to a root message.
    parent_id           UUID        DEFAULT NULL REFERENCES workspace_review_comments (id) ON DELETE CASCADE,

    -- The review this message belongs to; deleting the review removes its comments
    -- (CASCADE), so deleting a review takes its messages. Workspace scope is the
    -- review's (comment -> review -> workspace), not duplicated here.
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
COMMENT ON COLUMN workspace_review_comments.review_id IS 'Review this message belongs to';
COMMENT ON COLUMN workspace_review_comments.author_account_id IS 'Account that wrote the message';
COMMENT ON COLUMN workspace_review_comments.body IS 'Message text (1-10000 chars)';
COMMENT ON COLUMN workspace_review_comments.created_at IS 'Timestamp when the comment was posted';
COMMENT ON COLUMN workspace_review_comments.updated_at IS 'Timestamp of the last edit';
COMMENT ON COLUMN workspace_review_comments.deleted_at IS 'Soft-deletion timestamp; NULL means live';

-- Review assignees: the reviewers responsible for a review (0..N), and their
-- approvals. Assignment is metadata (who is responsible) and does not change the
-- review's status. A reviewer approves by stamping `approved_at` on their own row;
-- the review reaches `approved` once the count of stamped rows meets the workspace's
-- min_approvals. Requesting changes or reopening clears every `approved_at` (a
-- pushback invalidates accumulated approvals), so a stamp always means "approved the
-- state as it stands now."
CREATE TABLE workspace_review_assignees (
    review_id   UUID NOT NULL REFERENCES workspace_reviews (id) ON DELETE CASCADE,
    account_id  UUID NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT current_timestamp,
    -- When this reviewer approved the review in its current state; NULL means
    -- assigned but not yet approved.
    approved_at TIMESTAMPTZ DEFAULT NULL,
    PRIMARY KEY (review_id, account_id)
);

-- A reviewer's assigned reviews (backs the "my reviews" queue).
CREATE INDEX workspace_review_assignees_account_idx
    ON workspace_review_assignees (account_id);

COMMENT ON TABLE workspace_review_assignees IS 'The reviewers assigned to a review (many-to-many, 0..N) and their approvals (approved_at). Approval is reached when the count of stamped rows meets the workspace min_approvals.';
COMMENT ON COLUMN workspace_review_assignees.review_id IS 'The review';
COMMENT ON COLUMN workspace_review_assignees.account_id IS 'The assigned reviewer';
COMMENT ON COLUMN workspace_review_assignees.approved_at IS 'When this reviewer approved the current state; NULL means not yet approved. Cleared on request-changes/reopen';
COMMENT ON COLUMN workspace_review_assignees.created_at IS 'When the reviewer was assigned';

-- Kind of a review timeline event (a non-message entry in a review's stream): the
-- lifecycle (opened, renamed, submitted, changes requested, approved, reopened) and
-- the surrounding workflow (detection repoints, assignment changes). The approved
-- event's target carries the output redaction id.
-- Comments are the message entries; these are everything else in the one timeline.
CREATE TYPE REVIEW_EVENT_KIND AS ENUM (
    'review.opened',            -- The review was opened (target holds the initial detection id, if any)
    'review.renamed',           -- The review's display name was changed
    'review.redetected',        -- The review was repointed at a newer detection (target holds it)
    'review.assigned',          -- The review was assigned to a reviewer
    'review.unassigned',        -- A reviewer was unassigned
    'review.submitted',         -- The review was submitted (or resubmitted) for review
    'review.changes_requested', -- A reviewer requested changes
    'review.approved',          -- A reviewer approved (its target holds the output redaction id)
    'review.reopened'           -- An approved review was reopened (back to in_review)
);

COMMENT ON TYPE REVIEW_EVENT_KIND IS 'The kind of a non-message entry in a review timeline: the lifecycle (opened, renamed, submitted, changes_requested, approved, reopened) and the workflow (detection linked, assigned/unassigned).';

-- Review timeline events: the non-message entries in a review's stream. Immutable —
-- an event is a fact that happened, so there is no update or soft-delete; deleting
-- the review removes them. The reader merges these with the review's comments into
-- one timeline.
CREATE TABLE workspace_review_events (
    -- Primary identifier
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- The review this event belongs to; deleting the review removes its events.
    -- Workspace scope is the review's (event -> review -> workspace), not
    -- duplicated here.
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

COMMENT ON TABLE workspace_review_events IS 'An immutable non-message entry in a review timeline: the lifecycle (opened, renamed, verified, reopened) and the sign-off workflow (detection/redaction linked, assigned/unassigned).';
COMMENT ON COLUMN workspace_review_events.id IS 'Unique event identifier';
COMMENT ON COLUMN workspace_review_events.review_id IS 'Review this event belongs to';
COMMENT ON COLUMN workspace_review_events.kind IS 'What happened (review.opened/renamed/redetected, review.assigned/unassigned, review.submitted/changes_requested/approved/reopened)';
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
ALTER TYPE ACTIVITY_TYPE ADD VALUE IF NOT EXISTS 'review.submitted';
ALTER TYPE ACTIVITY_TYPE ADD VALUE IF NOT EXISTS 'review.changes_requested';
ALTER TYPE ACTIVITY_TYPE ADD VALUE IF NOT EXISTS 'review.approved';
ALTER TYPE ACTIVITY_TYPE ADD VALUE IF NOT EXISTS 'review.reopened';
ALTER TYPE ACTIVITY_TYPE ADD VALUE IF NOT EXISTS 'review.assigned';
ALTER TYPE ACTIVITY_TYPE ADD VALUE IF NOT EXISTS 'review.unassigned';

-- Webhooks carry the review lifecycle (message-level noise is left off).
ALTER TYPE WEBHOOK_EVENT ADD VALUE IF NOT EXISTS 'review.opened';
ALTER TYPE WEBHOOK_EVENT ADD VALUE IF NOT EXISTS 'review.renamed';
ALTER TYPE WEBHOOK_EVENT ADD VALUE IF NOT EXISTS 'review.submitted';
ALTER TYPE WEBHOOK_EVENT ADD VALUE IF NOT EXISTS 'review.changes_requested';
ALTER TYPE WEBHOOK_EVENT ADD VALUE IF NOT EXISTS 'review.approved';
ALTER TYPE WEBHOOK_EVENT ADD VALUE IF NOT EXISTS 'review.reopened';
ALTER TYPE WEBHOOK_EVENT ADD VALUE IF NOT EXISTS 'review.assigned';
ALTER TYPE WEBHOOK_EVENT ADD VALUE IF NOT EXISTS 'review.unassigned';

-- In-app notifications go to each mentioned account and to a newly-assigned reviewer.
ALTER TYPE NOTIFICATION_EVENT ADD VALUE IF NOT EXISTS 'comment.mentioned';
ALTER TYPE NOTIFICATION_EVENT ADD VALUE IF NOT EXISTS 'review.assigned';
