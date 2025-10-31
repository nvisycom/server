-- Drop all objects created in the documents migration
-- Drop in reverse order of creation to avoid dependency issues

-- Drop triggers first (they depend on functions and tables)
DROP TRIGGER IF EXISTS document_versions_auto_delete_trigger ON document_versions;
DROP TRIGGER IF EXISTS document_files_auto_delete_trigger ON document_files;

-- Drop functions
DROP FUNCTION IF EXISTS find_duplicate_files(_document_id UUID);
DROP FUNCTION IF EXISTS cleanup_expired_documents();
DROP FUNCTION IF EXISTS create_document_version(
    _document_id UUID,
    _account_id UUID
);
DROP FUNCTION IF EXISTS set_document_version_auto_delete();
DROP FUNCTION IF EXISTS set_document_file_auto_delete();

-- Drop views
DROP VIEW IF EXISTS processing_queue;
DROP VIEW IF EXISTS document_processing_summary;

-- Drop tables (indexes and remaining triggers dropped automatically with tables)
DROP TABLE IF EXISTS document_versions;
DROP TABLE IF EXISTS document_files;
DROP TABLE IF EXISTS documents;

-- Drop enum types
DROP TYPE IF EXISTS VIRUS_SCAN_STATUS;
DROP TYPE IF EXISTS REQUIRE_MODE;
DROP TYPE IF EXISTS PROCESSING_STATUS;
DROP TYPE IF EXISTS DOCUMENT_STATUS;
