-- Revert the storage split.
-- Objects are dropped in reverse order of creation.

DROP TABLE IF EXISTS workspace_documents;

DROP TYPE IF EXISTS DOCUMENT_KIND;

DROP TABLE IF EXISTS workspace_blobs;
