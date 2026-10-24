-- Revert the detections tables.
-- Objects are dropped in reverse order of creation.

DROP TABLE IF EXISTS workspace_detection_policy_versions;
DROP TABLE IF EXISTS workspace_detection_jobs;
DROP TABLE IF EXISTS workspace_detection_call_meter;
DROP TABLE IF EXISTS workspace_detection_call;
DROP TABLE IF EXISTS workspace_detections;

DROP TYPE IF EXISTS USAGE_METER;
DROP TYPE IF EXISTS DETECTION_STATUS;
