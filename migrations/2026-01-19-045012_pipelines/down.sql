-- Revert pipeline tables

-- Artifacts
DROP TABLE IF EXISTS workspace_pipeline_artifacts;
DROP TYPE IF EXISTS ARTIFACT_TYPE;

-- Views
DROP VIEW IF EXISTS workspace_pipeline_run_history;
DROP VIEW IF EXISTS active_workspace_pipeline_runs;

-- Pipeline runs
DROP TABLE IF EXISTS workspace_pipeline_runs;

-- Workspace pipelines
DROP TABLE IF EXISTS workspace_pipelines;

DROP TYPE IF EXISTS PIPELINE_TRIGGER_TYPE;
DROP TYPE IF EXISTS PIPELINE_RUN_STATUS;
DROP TYPE IF EXISTS PIPELINE_STATUS;

-- Workspace connections
DROP TABLE IF EXISTS workspace_connections;

DROP TYPE IF EXISTS SYNC_STATUS;
