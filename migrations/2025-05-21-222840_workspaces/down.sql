-- Revert the workspaces tables.
-- Objects are dropped in reverse order of creation.

DROP TABLE IF EXISTS workspace_invites;
DROP TABLE IF EXISTS workspace_members;
DROP TABLE IF EXISTS workspaces;

DROP TYPE IF EXISTS INVITE_STATUS;
DROP TYPE IF EXISTS WORKSPACE_ROLE;
