-- Revert the redactions table and the audit→redaction link added here.
-- Objects are dropped in reverse order of creation.

DROP INDEX IF EXISTS workspace_audits_redaction_idx;

ALTER TABLE workspace_audits
    DROP CONSTRAINT IF EXISTS workspace_audits_redaction_id_fkey;

DROP TABLE IF EXISTS workspace_redactions;
