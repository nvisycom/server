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
    -- "gone" at read time). The review audit — the engine's Audit after the
    -- reviewer edits were applied — is its own row in workspace_audits pointing
    -- back here (redaction_id), so there is no review column.
    output_document_id  UUID                DEFAULT NULL REFERENCES workspace_documents (id) ON DELETE SET NULL,

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
COMMENT ON COLUMN workspace_redactions.output_document_id IS 'Redacted document this redaction produced';
COMMENT ON COLUMN workspace_redactions.created_at IS 'When the redaction was created';

-- The workspace_audits table lives with the detections migration (its always-set
-- parent is detection_id). The redaction link cannot be declared there because
-- workspace_redactions does not exist yet, so it is added here: a review audit's
-- redaction_id names the redaction that produced it.
ALTER TABLE workspace_audits
    ADD CONSTRAINT workspace_audits_redaction_id_fkey
    FOREIGN KEY (redaction_id) REFERENCES workspace_redactions (id) ON DELETE CASCADE;

-- A redaction's review audit.
CREATE INDEX workspace_audits_redaction_idx
    ON workspace_audits (redaction_id)
    WHERE redaction_id IS NOT NULL;

-- Redaction creation feeds the activity log, webhooks, and in-app notifications.
ALTER TYPE ACTIVITY_TYPE ADD VALUE IF NOT EXISTS 'pipeline.redaction.created';
ALTER TYPE WEBHOOK_EVENT ADD VALUE IF NOT EXISTS 'pipeline.redaction.created';
ALTER TYPE NOTIFICATION_EVENT ADD VALUE IF NOT EXISTS 'pipeline.redaction.created';
