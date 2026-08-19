-- Revert the workspaces tables.
-- Objects are dropped in reverse order of creation.

DROP VIEW IF EXISTS pending_workspace_invites;
DROP VIEW IF EXISTS workspace_member_summary;

DROP TABLE IF EXISTS workspace_invites;
DROP TABLE IF EXISTS workspace_members;
DROP TABLE IF EXISTS workspaces;

DROP FUNCTION IF EXISTS cleanup_expired_invites;

DROP TYPE IF EXISTS INVITE_STATUS;
DROP TYPE IF EXISTS WORKSPACE_ROLE;
