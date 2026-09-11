-- Assistant: the reserved AI assistant account and the transactional outbox that
-- queues its replies. A user addresses the assistant (@assistant) in a thread,
-- and a background worker posts the model's reply as a comment authored by this
-- account, so it is a real `accounts` row that the existing author foreign key,
-- account-reference resolution, and timeline rendering all handle unchanged.

-- The reserved assistant account. It is not a person and not a workspace member:
-- it has no `account_identities` row, so no credential can authenticate as it and
-- it never uses the HTTP write path (the worker writes its comments
-- server-internally). Its id is a fixed, well-known constant (mirrored in the
-- Rust layer as ASSISTANT_ACCOUNT_ID) so code references it without a lookup.
--
-- The `accounts` table has case-insensitive partial unique indexes on
-- `lower(username)` and `lower(email_address)` (where `deleted_at IS NULL`), which
-- `ON CONFLICT (id)` does not cover. A live account under a *different* id already
-- holding this username or email would make a bare insert fail with an opaque
-- index violation, so guard the insert: skip it if the reserved id already exists,
-- and otherwise fail with a clear, actionable message if the reserved identifiers
-- are taken by another live account (the reserved handle/email must be free).
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM accounts WHERE id = '00000000-0000-0000-0000-000000000a11'
    ) THEN
        RETURN;
    END IF;

    IF EXISTS (
        SELECT 1 FROM accounts
        WHERE deleted_at IS NULL
          AND (lower(username) = 'assistant'
               OR lower(email_address) = 'assistant@nvisy.com')
    ) THEN
        RAISE EXCEPTION
            'Cannot create the reserved assistant account: the username '
            '"assistant" or email "assistant@nvisy.com" is already '
            'held by another live account. Free those identifiers, then re-run.';
    END IF;

    INSERT INTO accounts (id, is_verified, username, display_name, email_address)
    VALUES (
        '00000000-0000-0000-0000-000000000a11',
        TRUE,
        'assistant',
        'Assistant',
        'assistant@nvisy.com'
    );
END
$$;

-- Assistant-reply outbox: when a user posts a comment addressing the assistant, a
-- job row is inserted in the same transaction as the comment, then relayed by the
-- assistant drainer to the assistant NATS work-queue. A worker runs the model
-- over the thread's conversation and posts the reply. Mirrors the detection-job
-- outbox: at-least-once delivery, deduped by the worker.
CREATE TABLE workspace_assistant_jobs (
    -- Primary identifier
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References. The comment that triggered this reply (the message addressing
    -- the assistant); the row is deleted with its comment.
    comment_id      UUID          NOT NULL REFERENCES workspace_thread_comments (id) ON DELETE CASCADE,

    -- The job: a serialized `AssistantJob` (the workspace, thread, and triggering
    -- comment) the drainer publishes to the worker.
    job             JSONB         NOT NULL,
    CONSTRAINT workspace_assistant_jobs_job_size CHECK (length(job::TEXT) BETWEEN 2 AND 16384),

    -- Drainer bookkeeping: processing state, publish attempts, and the earliest
    -- time the row may next be claimed (advanced by a backoff on each failed
    -- attempt so a failing row does not spin at the head of the queue).
    status          OUTBOX_STATUS NOT NULL DEFAULT 'pending',
    attempts        INTEGER       NOT NULL DEFAULT 0,
    CONSTRAINT workspace_assistant_jobs_attempts_non_negative CHECK (attempts >= 0),
    next_attempt_at TIMESTAMPTZ   NOT NULL DEFAULT current_timestamp,

    -- Lifecycle timestamps
    created_at      TIMESTAMPTZ   NOT NULL DEFAULT current_timestamp,
    resolved_at     TIMESTAMPTZ   DEFAULT NULL,
    CONSTRAINT workspace_assistant_jobs_resolved_only_when_terminal
        CHECK (resolved_at IS NULL OR status IN ('processed', 'failed')),
    CONSTRAINT workspace_assistant_jobs_resolved_after_created
        CHECK (resolved_at IS NULL OR resolved_at >= created_at)
);

-- The drainer's claim queue: pending rows ordered by due time then age. Partial
-- so it stays small as processed and failed rows accumulate.
CREATE INDEX workspace_assistant_jobs_pending_idx
    ON workspace_assistant_jobs (next_attempt_at, created_at)
    WHERE status = 'pending';

-- Back the comment foreign key so a comment delete cascades without scanning the
-- whole outbox (Postgres does not index a referencing column automatically, and
-- the partial claim index above does not cover it).
CREATE INDEX workspace_assistant_jobs_comment_idx
    ON workspace_assistant_jobs (comment_id);

COMMENT ON TABLE workspace_assistant_jobs IS 'Transactional outbox of assistant-reply jobs, drained to the assistant NATS work-queue.';
COMMENT ON COLUMN workspace_assistant_jobs.id IS 'Unique outbox row identifier';
COMMENT ON COLUMN workspace_assistant_jobs.comment_id IS 'Comment that addressed the assistant and triggered this reply';
COMMENT ON COLUMN workspace_assistant_jobs.job IS 'Serialized AssistantJob published to the worker (JSON, 2B-16KB)';
COMMENT ON COLUMN workspace_assistant_jobs.status IS 'Processing state: pending, processed, or failed (dead-lettered)';
COMMENT ON COLUMN workspace_assistant_jobs.attempts IS 'Number of publish attempts the drainer has made';
COMMENT ON COLUMN workspace_assistant_jobs.next_attempt_at IS 'Earliest time the row may next be claimed; advanced by a backoff after each failed attempt';
COMMENT ON COLUMN workspace_assistant_jobs.created_at IS 'Timestamp when the job was queued';
COMMENT ON COLUMN workspace_assistant_jobs.resolved_at IS 'When a terminal (processed or failed) row was resolved by an operator; NULL until then';
