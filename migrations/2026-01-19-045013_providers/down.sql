-- Revert the providers table.
-- Objects are dropped in reverse order of creation.

DROP TABLE IF EXISTS workspace_providers;

DROP TYPE IF EXISTS PROVIDER_TYPE;
