-- Revert connections

DROP VIEW IF EXISTS workspace_connection_sync_state;

DROP TABLE IF EXISTS workspace_connection_runs;

DROP TABLE IF EXISTS workspace_connections;

DROP TYPE IF EXISTS SYNC_TRIGGER_TYPE;

DROP TYPE IF EXISTS SYNC_STATUS;
