-- Revert connections

DROP VIEW IF EXISTS workspace_connection_sync_state;

DROP TABLE IF EXISTS workspace_connection_runs;

-- Drop the run_number trigger function (its trigger dropped with the table above).
DROP FUNCTION IF EXISTS set_workspace_connection_run_number();

DROP TABLE IF EXISTS workspace_connections;

DROP TYPE IF EXISTS SYNC_TRIGGER_TYPE;

DROP TYPE IF EXISTS SYNC_STATUS;
