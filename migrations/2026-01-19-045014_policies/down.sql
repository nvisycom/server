-- Revert the policies tables.
ALTER TABLE IF EXISTS workspace_policies DROP CONSTRAINT IF EXISTS workspace_policies_current_version_fkey;
DROP TABLE IF EXISTS workspace_policy_versions;
DROP TABLE IF EXISTS workspace_policies;
