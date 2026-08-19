-- Revert the accounts tables.
-- Objects are dropped in reverse order of creation.

DROP VIEW IF EXISTS active_user_sessions;

DROP TABLE IF EXISTS account_api_tokens;
DROP TABLE IF EXISTS accounts;

DROP FUNCTION IF EXISTS cleanup_expired_auth_data;

DROP TYPE IF EXISTS API_TOKEN_TYPE;
