-- Revert the files table.
-- Objects are dropped in reverse order of creation.

DROP TABLE IF EXISTS workspace_files;

DROP FUNCTION IF EXISTS set_workspace_file_version_number;

DROP TYPE IF EXISTS FILE_KIND;
