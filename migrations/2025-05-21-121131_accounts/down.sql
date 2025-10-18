-- Drop all objects created in the accounts migration
-- Drop in reverse order of creation to avoid dependency issues

DROP FUNCTION IF EXISTS cleanup_expired_auth_data();

DROP VIEW IF EXISTS active_user_sessions;

DROP TABLE IF EXISTS account_action_tokens;
DROP TABLE IF EXISTS account_api_tokens;
DROP TABLE IF EXISTS accounts;

DROP TYPE IF EXISTS ACTION_TOKEN_TYPE;
DROP TYPE IF EXISTS API_TOKEN_TYPE;
