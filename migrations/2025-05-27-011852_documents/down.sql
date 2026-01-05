-- Drop all objects created in the documents migration
-- Drop in reverse order of creation to avoid dependency issues

-- Drop functions
DROP FUNCTION IF EXISTS find_duplicate_files(_document_id UUID);

-- Drop views
DROP VIEW IF EXISTS processing_queue;
DROP VIEW IF EXISTS document_processing_summary;

-- Drop tables (indexes and remaining triggers dropped automatically with tables)
DROP TABLE IF EXISTS document_annotations;
DROP TABLE IF EXISTS document_comments;
DROP TABLE IF EXISTS document_chunks;
DROP TABLE IF EXISTS document_files;
DROP TABLE IF EXISTS documents;

-- Drop enum types
DROP TYPE IF EXISTS ANNOTATION_TYPE;
DROP TYPE IF EXISTS CONTENT_SEGMENTATION;
DROP TYPE IF EXISTS REQUIRE_MODE;
DROP TYPE IF EXISTS PROCESSING_STATUS;
DROP TYPE IF EXISTS DOCUMENT_STATUS;
