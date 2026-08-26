-- Revert the detections tables.
-- Objects are dropped in reverse order of creation.

DROP TABLE IF EXISTS workspace_detection_jobs;
DROP TABLE IF EXISTS workspace_detection_usage;
DROP TABLE IF EXISTS workspace_detections;

DROP TYPE IF EXISTS PIPELINE_TRIGGER_TYPE;
DROP TYPE IF EXISTS DETECTION_STATUS;
