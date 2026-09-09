-- Revert the connections tables.
-- Objects are dropped in reverse order of creation.

DROP TABLE IF EXISTS workspace_file_exports;
DROP TABLE IF EXISTS workspace_file_imports;
DROP TABLE IF EXISTS workspace_connection_syncs;
DROP TABLE IF EXISTS workspace_connection_schedule;
DROP TABLE IF EXISTS workspace_connections;

DROP TYPE IF EXISTS CONNECTION_TYPE;
DROP TYPE IF EXISTS SYNC_DELETION_POLICY;
DROP TYPE IF EXISTS SYNC_MODE;
DROP TYPE IF EXISTS SYNC_TRIGGER_TYPE;
DROP TYPE IF EXISTS SYNC_STATUS;
