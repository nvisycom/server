-- Revert pipeline tables

-- Artifacts
DROP TABLE IF EXISTS pipeline_artifacts;
DROP TYPE IF EXISTS ARTIFACT_TYPE;

-- Views
DROP VIEW IF EXISTS pipeline_run_history;
DROP VIEW IF EXISTS active_pipeline_runs;

-- Pipeline runs
DROP TABLE IF EXISTS pipeline_runs;

-- Workspace pipelines
DROP TABLE IF EXISTS workspace_pipelines;

DROP TYPE IF EXISTS PIPELINE_TRIGGER_TYPE;
DROP TYPE IF EXISTS PIPELINE_RUN_STATUS;
DROP TYPE IF EXISTS PIPELINE_STATUS;

-- Workspace connections
DROP TABLE IF EXISTS workspace_connections;
