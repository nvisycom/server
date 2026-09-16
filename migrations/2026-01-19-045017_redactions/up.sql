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

    -- The redacted document this redaction produced. A redaction always yields
    -- one; it is nullable only because `ON DELETE SET NULL` clears the reference
    -- if the document is ever hard-deleted (a soft-deleted document resolves to
    -- "gone" at read time).
    output_document_id  UUID                DEFAULT NULL REFERENCES workspace_documents (id) ON DELETE SET NULL,

    -- The review audit: the engine's findings set after the reviewer's edits were
    -- applied and the per-entity redaction outcome recorded, stored as bytes in a
    -- blob. NULL once the bytes are reclaimed on retention (the redaction row — a
    -- ledger record — is kept). Served as the redaction's review view.
    review_audit_blob_id UUID               DEFAULT NULL REFERENCES workspace_blobs (id) ON DELETE SET NULL,

    -- Timing
    created_at      TIMESTAMPTZ             NOT NULL DEFAULT current_timestamp,

    -- Soft-delete. A redaction is a ledger record (it ran, it cost credits), so
    -- deleting one keeps the row and only releases its blob bytes; the row is hard-
    -- removed only when its workspace is purged.
    deleted_at      TIMESTAMPTZ             DEFAULT NULL,
    CONSTRAINT workspace_redactions_deleted_after_created CHECK (deleted_at IS NULL OR deleted_at >= created_at)
);

-- A detection's redactions, newest first (the redaction list).
CREATE INDEX workspace_redactions_detection_idx
    ON workspace_redactions (detection_id, created_at DESC)
    WHERE deleted_at IS NULL;

-- Redactions requested by an account, newest first.
CREATE INDEX workspace_redactions_account_idx
    ON workspace_redactions (account_id, created_at DESC)
    WHERE deleted_at IS NULL;

-- Back the review-audit blob reference, walked by the retention sweep that
-- reclaims the review bytes and nulls this pointer.
CREATE INDEX workspace_redactions_review_audit_blob_idx
    ON workspace_redactions (review_audit_blob_id)
    WHERE review_audit_blob_id IS NOT NULL;

COMMENT ON TABLE workspace_redactions IS 'Redactions: one redact pass over a detection, with its own reviewer edits, review audit, and output.';
COMMENT ON COLUMN workspace_redactions.id IS 'Unique redaction identifier';
COMMENT ON COLUMN workspace_redactions.detection_id IS 'Detection this redaction was produced from';
COMMENT ON COLUMN workspace_redactions.account_id IS 'Account that requested the redaction';
COMMENT ON COLUMN workspace_redactions.output_document_id IS 'Redacted document this redaction produced';
COMMENT ON COLUMN workspace_redactions.review_audit_blob_id IS 'Review analysis (findings after reviewer edits) blob; NULL once the bytes are reclaimed on retention (the row is kept)';
COMMENT ON COLUMN workspace_redactions.created_at IS 'When the redaction was created';
COMMENT ON COLUMN workspace_redactions.deleted_at IS 'Soft-deletion timestamp; NULL means live. The row is kept as a ledger record; only its blob bytes are reclaimed';

-- Redaction creation feeds the activity log, webhooks, and in-app notifications.
ALTER TYPE ACTIVITY_TYPE ADD VALUE IF NOT EXISTS 'pipeline.redaction.created';
ALTER TYPE WEBHOOK_EVENT ADD VALUE IF NOT EXISTS 'pipeline.redaction.created';
ALTER TYPE NOTIFICATION_EVENT ADD VALUE IF NOT EXISTS 'pipeline.redaction.created';
