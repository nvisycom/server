-- Drop all objects created in the projects migration
-- Drop in reverse order of creation to avoid dependency issues

-- Drop functions
DROP FUNCTION IF EXISTS cleanup_expired_invites();

-- Drop views
DROP VIEW IF EXISTS pending_project_invites;
DROP VIEW IF EXISTS project_member_summary;

-- Drop tables (indexes and triggers dropped automatically with tables)
DROP TABLE IF EXISTS project_runs;
DROP TABLE IF EXISTS project_templates;
DROP TABLE IF EXISTS project_pipelines;
DROP TABLE IF EXISTS project_webhooks;
DROP TABLE IF EXISTS project_integrations;
DROP TABLE IF EXISTS project_activities;
DROP TABLE IF EXISTS project_invites;
DROP TABLE IF EXISTS project_members;
DROP TABLE IF EXISTS projects;

-- Drop enum types
DROP TYPE IF EXISTS ACTIVITY_TYPE;
DROP TYPE IF EXISTS WEBHOOK_STATUS;
DROP TYPE IF EXISTS INTEGRATION_TYPE;
DROP TYPE IF EXISTS INTEGRATION_STATUS;
DROP TYPE IF EXISTS INVITE_STATUS;
DROP TYPE IF EXISTS PROJECT_ROLE;
DROP TYPE IF EXISTS PROJECT_VISIBILITY;
DROP TYPE IF EXISTS PROJECT_STATUS;
