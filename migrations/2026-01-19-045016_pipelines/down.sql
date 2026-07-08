-- Revert pipeline tables

-- Pipeline → policy / context join tables (drop before their parents)
DROP TABLE IF EXISTS pipeline_contexts;
DROP TABLE IF EXISTS pipeline_policies;

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
