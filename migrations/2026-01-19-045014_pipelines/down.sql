-- Revert the pipelines tables.
-- Objects are dropped in reverse order of creation.

DROP TABLE IF EXISTS workspace_redactions;
DROP TABLE IF EXISTS workspace_detection_usage;
DROP TABLE IF EXISTS workspace_detections;
DROP TABLE IF EXISTS workspace_pipelines;

DROP TYPE IF EXISTS PIPELINE_TRIGGER_TYPE;
DROP TYPE IF EXISTS DETECTION_STATUS;
DROP TYPE IF EXISTS PIPELINE_STATUS;
