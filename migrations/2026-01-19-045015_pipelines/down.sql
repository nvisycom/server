-- Revert the pipelines tables.
-- Objects are dropped in reverse order of creation.

DROP TABLE IF EXISTS workspace_pipeline_policies;
DROP TABLE IF EXISTS workspace_pipelines;

DROP TYPE IF EXISTS PIPELINE_STATUS;
