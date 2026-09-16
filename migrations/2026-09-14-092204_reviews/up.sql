-- Document reviews: an optional, purpose-scoped sign-off effort on a document.
--
-- A document has 0..N reviews — none when someone works alone, one or more when a
-- formal sign-off is wanted, each for a distinct purpose (an "audience": public,
-- legal, internal, …). A review owns a discussion thread (workspace_threads) and
-- references — does not own — the detections and redactions done for its purpose,
-- via the link tables here. Its state is a manual reviewer workflow, and its own
-- activity (links, assignment, verification) is logged in workspace_review_events,
-- kept separate from the thread's discussion timeline.
--
-- This subsystem sits on top of threads, documents, detections, and redactions
-- (all introduced by earlier migrations).

-- The state of a document review. A manual reviewer workflow: a review opens at
-- `needs_review`, moves to `in_review` when a reviewer takes it, and `resolved`
-- when verified (reopen returns it to `needs_review`).
CREATE TYPE REVIEW_STATUS AS ENUM (
    'needs_review', -- Opened, awaiting a reviewer
    'in_review',    -- A reviewer has taken it (assigned/started)
    'resolved'      -- Verified
);

COMMENT ON TYPE REVIEW_STATUS IS 'The state of a document review: needs_review, in_review, or resolved.';

-- Document reviews: an optional, purpose-scoped sign-off effort on a document. A
-- document has 0..N reviews — none when someone works alone, one or more when a
-- formal sign-off is wanted, each for a distinct purpose (an "audience": public,
-- legal, internal, …). A review owns a discussion thread and carries its state;
-- it references (does not own) its assignees and the detections and redactions
-- done for its purpose via the link tables below.
CREATE TABLE workspace_reviews (
    -- Primary identifier
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References. Denormalized workspace scope, the document under review, and the
    -- discussion thread this review owns. The document is referenced with its
    -- workspace against workspace_documents (workspace_id, id), so the denormalized
    -- workspace_id must match the document's own; removing the document (or its
    -- thread) cascades the review away.
    workspace_id        UUID        NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    document_id         UUID        NOT NULL,
    thread_id           UUID        NOT NULL REFERENCES workspace_threads (id) ON DELETE CASCADE,

    -- Optional free-text label for what this review is for (the "audience" or
    -- purpose): "Public release", "Court filing". Descriptive metadata, not a key.
    purpose             TEXT        DEFAULT NULL,
    CONSTRAINT workspace_reviews_purpose_length CHECK (purpose IS NULL OR length(trim(purpose)) BETWEEN 1 AND 255),

    -- The review state (manual reviewer workflow). Assignees live in the
    -- workspace_review_assignees link table (a review may have 0..N reviewers).
    review_status       REVIEW_STATUS NOT NULL DEFAULT 'needs_review',

    -- Lifecycle timestamps
    created_at          TIMESTAMPTZ NOT NULL DEFAULT current_timestamp,
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT current_timestamp,
    CONSTRAINT workspace_reviews_updated_after_created CHECK (updated_at >= created_at),

    -- A review owns exactly one discussion thread (but a document may have many
    -- reviews, so document_id is deliberately not unique).
    CONSTRAINT workspace_reviews_thread_key UNIQUE (thread_id),
    CONSTRAINT workspace_reviews_document_fkey FOREIGN KEY (workspace_id, document_id)
        REFERENCES workspace_documents (workspace_id, id) ON DELETE CASCADE
);

-- A document's reviews, newest first (backs the per-document review listing).
CREATE INDEX workspace_reviews_document_idx
    ON workspace_reviews (document_id, created_at DESC);

-- The review queue: a workspace's reviews by status, newest first (backs the
-- status-filtered listing and the "needs review" queue).
CREATE INDEX workspace_reviews_workspace_idx
    ON workspace_reviews (workspace_id, review_status, created_at DESC);

-- Keep updated_at current on every row modification. Reviews have no `deleted_at`
-- (they cascade with their document/thread), so they use the no-soft-delete
-- trigger variant.
SELECT setup_updated_at_no_soft_delete('workspace_reviews');

COMMENT ON TABLE workspace_reviews IS 'An optional, purpose-scoped sign-off effort on a document (0..N per document). Owns a discussion thread; references the detections/redactions for its purpose via link tables.';
COMMENT ON COLUMN workspace_reviews.id IS 'Unique review identifier';
COMMENT ON COLUMN workspace_reviews.workspace_id IS 'Denormalized workspace scope';
COMMENT ON COLUMN workspace_reviews.document_id IS 'Document under review (a document may have many reviews)';
COMMENT ON COLUMN workspace_reviews.thread_id IS 'Discussion thread this review owns (one review per thread)';
COMMENT ON COLUMN workspace_reviews.purpose IS 'Optional free-text label for the review''s purpose/audience (1-255 chars)';
COMMENT ON COLUMN workspace_reviews.review_status IS 'Review state (needs_review/in_review/resolved)';
COMMENT ON COLUMN workspace_reviews.created_at IS 'Timestamp when the review was created';
COMMENT ON COLUMN workspace_reviews.updated_at IS 'Timestamp of the last update';

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

-- Review ← detection links: the detections a review references for its purpose.
-- Many-to-many — a shared detection's findings can feed redactions across several
-- purpose-reviews, so a detection may be referenced by more than one review.
CREATE TABLE workspace_review_detections (
    review_id    UUID NOT NULL REFERENCES workspace_reviews (id) ON DELETE CASCADE,
    detection_id UUID NOT NULL REFERENCES workspace_detections (id) ON DELETE CASCADE,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT current_timestamp,
    PRIMARY KEY (review_id, detection_id)
);

-- Reverse lookup: which reviews reference a given detection.
CREATE INDEX workspace_review_detections_detection_idx
    ON workspace_review_detections (detection_id);

COMMENT ON TABLE workspace_review_detections IS 'Links a review to the detections it references for its purpose (many-to-many).';
COMMENT ON COLUMN workspace_review_detections.review_id IS 'The referencing review';
COMMENT ON COLUMN workspace_review_detections.detection_id IS 'The referenced detection';
COMMENT ON COLUMN workspace_review_detections.created_at IS 'When the link was made';

-- Review ← redaction links: the redaction passes a review references for its
-- purpose. Many-to-many for the same reason as detections.
CREATE TABLE workspace_review_redactions (
    review_id    UUID NOT NULL REFERENCES workspace_reviews (id) ON DELETE CASCADE,
    redaction_id UUID NOT NULL REFERENCES workspace_redactions (id) ON DELETE CASCADE,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT current_timestamp,
    PRIMARY KEY (review_id, redaction_id)
);

-- Reverse lookup: which reviews reference a given redaction.
CREATE INDEX workspace_review_redactions_redaction_idx
    ON workspace_review_redactions (redaction_id);

COMMENT ON TABLE workspace_review_redactions IS 'Links a review to the redactions it references for its purpose (many-to-many).';
COMMENT ON COLUMN workspace_review_redactions.review_id IS 'The referencing review';
COMMENT ON COLUMN workspace_review_redactions.redaction_id IS 'The referenced redaction';
COMMENT ON COLUMN workspace_review_redactions.created_at IS 'When the link was made';

-- Kind of a review timeline event (the review's own activity log, distinct from
-- the discussion lifecycle in workspace_thread_events). Records what was done to
-- the review: artifacts linked, assignment changes, and the verification lifecycle.
CREATE TYPE REVIEW_EVENT_KIND AS ENUM (
    'detection.linked',   -- A detection was referenced by the review
    'redaction.linked',   -- A redaction was referenced by the review
    'assigned',           -- The review was assigned to a reviewer
    'unassigned',         -- The review's assignee was cleared
    'verified',           -- The review was verified (resolved)
    'reopened'            -- A resolved review was reopened
);

COMMENT ON TYPE REVIEW_EVENT_KIND IS 'The kind of a review timeline event: detection/redaction linked, assigned/unassigned, verified, reopened.';

-- Review timeline events: the review's activity log. Immutable — an event is a
-- fact that happened, so there is no update or soft-delete; deleting the review
-- removes them.
CREATE TABLE workspace_review_events (
    -- Primary identifier
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References. Denormalized workspace scope and the review this event belongs to.
    workspace_id        UUID        NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    review_id           UUID        NOT NULL REFERENCES workspace_reviews (id) ON DELETE CASCADE,

    -- What happened.
    kind                REVIEW_EVENT_KIND NOT NULL,

    -- Who did it. If their account is removed, keep the event but forget the actor.
    actor_account_id    UUID        DEFAULT NULL REFERENCES accounts (id) ON DELETE SET NULL,

    -- Event-specific detail, when any: the linked detection/redaction id, or the
    -- assignee for an assign. NULL otherwise.
    target              JSONB       DEFAULT NULL,
    CONSTRAINT workspace_review_events_target_size CHECK (target IS NULL OR length(target::TEXT) <= 8192),

    -- When it happened (events are immutable, so only a creation timestamp).
    created_at          TIMESTAMPTZ NOT NULL DEFAULT current_timestamp
);

-- A review's timeline events, oldest first.
CREATE INDEX workspace_review_events_review_idx
    ON workspace_review_events (review_id, created_at);

COMMENT ON TABLE workspace_review_events IS 'An immutable entry in a review''s activity log: detection/redaction linked, assigned/unassigned, verified, reopened.';
COMMENT ON COLUMN workspace_review_events.id IS 'Unique event identifier';
COMMENT ON COLUMN workspace_review_events.workspace_id IS 'Denormalized workspace scope';
COMMENT ON COLUMN workspace_review_events.review_id IS 'Review this event belongs to';
COMMENT ON COLUMN workspace_review_events.kind IS 'What happened (detection.linked/redaction.linked/assigned/unassigned/verified/reopened)';
COMMENT ON COLUMN workspace_review_events.actor_account_id IS 'Account that performed the action; null if that account was removed';
COMMENT ON COLUMN workspace_review_events.target IS 'Event-specific detail (linked id, assignee); NULL when none';
COMMENT ON COLUMN workspace_review_events.created_at IS 'Timestamp when the event happened';

-- Review lifecycle events feed the event sinks (the detection subsystem already
-- logs detection/redaction creation, so only the review's own gestures are added).
-- ALTER TYPE only adds labels (no rows use them yet), so it stays transactional.
ALTER TYPE ACTIVITY_TYPE ADD VALUE IF NOT EXISTS 'review.verified';
ALTER TYPE ACTIVITY_TYPE ADD VALUE IF NOT EXISTS 'review.assigned';
ALTER TYPE ACTIVITY_TYPE ADD VALUE IF NOT EXISTS 'review.unassigned';

ALTER TYPE WEBHOOK_EVENT ADD VALUE IF NOT EXISTS 'review.verified';
ALTER TYPE WEBHOOK_EVENT ADD VALUE IF NOT EXISTS 'review.assigned';
ALTER TYPE WEBHOOK_EVENT ADD VALUE IF NOT EXISTS 'review.unassigned';

-- A reviewer is notified in-app when a review is assigned to them.
ALTER TYPE NOTIFICATION_EVENT ADD VALUE IF NOT EXISTS 'review.assigned';
