-- Drop all objects created in the accounts migration
-- Drop in reverse order of creation to avoid dependency issues

-- Drop functions
DROP FUNCTION IF EXISTS cleanup_expired_auth_data();

-- Drop views
DROP VIEW IF EXISTS active_user_sessions;

-- Drop tables (indexes and triggers dropped automatically with tables)
DROP TABLE IF EXISTS account_action_tokens;
DROP TABLE IF EXISTS account_api_tokens;
DROP TABLE IF EXISTS accounts;

-- Drop enum types
DROP TYPE IF EXISTS ACTION_TOKEN_TYPE;
DROP TYPE IF EXISTS API_TOKEN_TYPE;
