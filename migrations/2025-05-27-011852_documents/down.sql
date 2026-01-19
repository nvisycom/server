-- Drop all objects created in the documents migration
-- Drop in reverse order of creation to avoid dependency issues

-- Drop tables (indexes dropped automatically with tables)
DROP TABLE IF EXISTS file_annotations;
DROP TABLE IF EXISTS file_chunks;

-- Drop trigger before the function it depends on
DROP TRIGGER IF EXISTS files_set_version_trigger ON files;

-- Drop files table
DROP TABLE IF EXISTS files;

-- Drop functions (after triggers that depend on them)
DROP FUNCTION IF EXISTS find_duplicate_files(UUID);
DROP FUNCTION IF EXISTS set_file_version_number();

-- Drop enum types
DROP TYPE IF EXISTS ANNOTATION_TYPE;
DROP TYPE IF EXISTS FILE_SOURCE;
