-- Drop all objects created in the workspaces migration
-- Drop in reverse order of creation to avoid dependency issues

-- Drop functions
DROP FUNCTION IF EXISTS cleanup_expired_invites();

-- Drop views
DROP VIEW IF EXISTS pending_workspace_invites;
DROP VIEW IF EXISTS workspace_member_summary;

-- Drop tables (indexes and triggers dropped automatically with tables)
DROP TABLE IF EXISTS workspace_integration_runs;
DROP TABLE IF EXISTS workspace_webhooks;
DROP TABLE IF EXISTS workspace_integrations;
DROP TABLE IF EXISTS workspace_activities;
DROP TABLE IF EXISTS workspace_invites;
DROP TABLE IF EXISTS workspace_members;
DROP TABLE IF EXISTS workspaces;

-- Drop enum types
DROP TYPE IF EXISTS ACTIVITY_TYPE;
DROP TYPE IF EXISTS WEBHOOK_EVENT;
DROP TYPE IF EXISTS WEBHOOK_STATUS;
DROP TYPE IF EXISTS INTEGRATION_TYPE;
DROP TYPE IF EXISTS INTEGRATION_STATUS;
DROP TYPE IF EXISTS INVITE_STATUS;
DROP TYPE IF EXISTS WORKSPACE_ROLE;
