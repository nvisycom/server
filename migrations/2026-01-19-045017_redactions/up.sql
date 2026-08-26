-- Redactions: one redact pass over a detection's analysis. A detection can be
-- redacted many times, so each redaction is its own row owning the review audit
-- it applied and the redacted document it produced.

CREATE TABLE workspace_redactions (
    -- Primary identifier
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- The detection this redaction was produced from; redactions are deleted
    -- with their detection.
    detection_id        UUID                NOT NULL REFERENCES workspace_detections (id) ON DELETE CASCADE,

    -- Account that requested the redaction.
    account_id          UUID                NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,

    -- The two files a redaction produces. A redaction always yields both, so the
    -- app sets them at creation; they are nullable only because `ON DELETE SET
    -- NULL` clears a reference if its file is ever hard-deleted (for the same
    -- append-only-history reasons as a detection — a soft-deleted file resolves to
    -- "gone" at read time, distinct from a NULL that means the file was purged).
    --   review: the engine's Audit after the reviewer edits were applied and
    --           redaction ran (`file_kind = review`); the record of exactly what
    --           was redacted and why.
    --   output: the redacted document this redaction produced.
    review_file_id  UUID                    DEFAULT NULL REFERENCES workspace_files (id) ON DELETE SET NULL,
    output_file_id  UUID                    DEFAULT NULL REFERENCES workspace_files (id) ON DELETE SET NULL,

    -- Timing
    created_at      TIMESTAMPTZ             NOT NULL DEFAULT current_timestamp
);

-- A detection's redactions, newest first (the redaction list).
CREATE INDEX workspace_redactions_detection_idx
    ON workspace_redactions (detection_id, created_at DESC);

-- Redactions requested by an account, newest first.
CREATE INDEX workspace_redactions_account_idx
    ON workspace_redactions (account_id, created_at DESC);

COMMENT ON TABLE workspace_redactions IS 'Redactions: one redact pass over a detection, with its own reviewer edits, review audit, and output.';
COMMENT ON COLUMN workspace_redactions.id IS 'Unique redaction identifier';
COMMENT ON COLUMN workspace_redactions.detection_id IS 'Detection this redaction was produced from';
COMMENT ON COLUMN workspace_redactions.account_id IS 'Account that requested the redaction';
COMMENT ON COLUMN workspace_redactions.review_file_id IS 'Review audit (file_kind=review) recording the applied edits and redaction outcome';
COMMENT ON COLUMN workspace_redactions.output_file_id IS 'Redacted document this redaction produced';
COMMENT ON COLUMN workspace_redactions.created_at IS 'When the redaction was created';
