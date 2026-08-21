-- Revert the pipelines tables.
-- Objects are dropped in reverse order of creation.

DROP TABLE IF EXISTS workspace_pipeline_run_usage;
DROP TABLE IF EXISTS workspace_pipeline_runs;
DROP TABLE IF EXISTS workspace_pipelines;

DROP TYPE IF EXISTS PIPELINE_TRIGGER_TYPE;
DROP TYPE IF EXISTS PIPELINE_RUN_STATUS;
DROP TYPE IF EXISTS PIPELINE_STATUS;
