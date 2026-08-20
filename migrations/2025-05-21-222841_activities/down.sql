-- Revert the activities table.
-- Objects are dropped in reverse order of creation.

DROP TABLE IF EXISTS workspace_activities;

DROP TYPE IF EXISTS ACTIVITY_TYPE;
