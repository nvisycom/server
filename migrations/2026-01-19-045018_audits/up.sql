-- Audits: the engine's findings set over a document, one row per audit. A base
-- audit is produced by a detection (redaction_id and derived_from NULL); a review
-- audit is produced by a redaction applying reviewer edits and carries redaction_id
-- plus derived_from (the base audit it was edited from). The bytes live in a blob;
-- this row is the provenance. Workspace scope is the detection's (audit ->
-- detection -> workspace), not duplicated here.

CREATE TABLE workspace_audits (
    -- Primary identifier
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References
    blob_id             UUID                NOT NULL REFERENCES workspace_blobs (id),

    -- The detection that produced the base analysis this audit belongs to (always
    -- set). A review audit additionally names the redaction that produced it and
    -- the base audit it was edited from; both NULL for a base audit.
    detection_id        UUID                NOT NULL REFERENCES workspace_detections (id) ON DELETE CASCADE,
    redaction_id        UUID                DEFAULT NULL REFERENCES workspace_redactions (id) ON DELETE CASCADE,
    derived_from        UUID                DEFAULT NULL REFERENCES workspace_audits (id) ON DELETE SET NULL,
    CONSTRAINT workspace_audits_review_consistent CHECK (
        (redaction_id IS NULL) = (derived_from IS NULL)
    ),

    -- Timing
    created_at          TIMESTAMPTZ         NOT NULL DEFAULT current_timestamp
);

-- A detection's audits, newest first (its base audit and every review derived
-- through its redactions).
CREATE INDEX workspace_audits_detection_idx
    ON workspace_audits (detection_id, created_at DESC);

-- Back the blob foreign key (ref-count maintenance and reclamation walks it).
CREATE INDEX workspace_audits_blob_idx
    ON workspace_audits (blob_id);

-- A redaction's review audit.
CREATE INDEX workspace_audits_redaction_idx
    ON workspace_audits (redaction_id)
    WHERE redaction_id IS NOT NULL;

COMMENT ON TABLE workspace_audits IS 'Findings sets over a document: a detection''s base audit and the review audits redactions derive from it, sharing one type via lineage.';
COMMENT ON COLUMN workspace_audits.id IS 'Unique audit identifier';
COMMENT ON COLUMN workspace_audits.blob_id IS 'Blob holding the findings bytes';
COMMENT ON COLUMN workspace_audits.detection_id IS 'Detection whose analysis this audit belongs to';
COMMENT ON COLUMN workspace_audits.redaction_id IS 'Redaction that produced this review audit; NULL for a base audit';
COMMENT ON COLUMN workspace_audits.derived_from IS 'Base audit this review was edited from; NULL for a base audit';
COMMENT ON COLUMN workspace_audits.created_at IS 'When the audit was created';
