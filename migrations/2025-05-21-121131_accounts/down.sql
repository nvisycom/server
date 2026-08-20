-- Revert the accounts tables.
-- Objects are dropped in reverse order of creation.

DROP TABLE IF EXISTS account_api_tokens;
DROP TABLE IF EXISTS accounts;

DROP TYPE IF EXISTS API_TOKEN_TYPE;
