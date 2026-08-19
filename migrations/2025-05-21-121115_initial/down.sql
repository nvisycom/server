-- Revert the initial utility functions and extensions.
-- Objects are dropped in reverse order of creation.

DROP FUNCTION IF EXISTS is_valid_email;
DROP FUNCTION IF EXISTS generate_secure_token;
DROP FUNCTION IF EXISTS cleanup_expired_records;
DROP FUNCTION IF EXISTS restore_record;
DROP FUNCTION IF EXISTS soft_delete_record;
DROP FUNCTION IF EXISTS setup_updated_at_no_soft_delete;
DROP FUNCTION IF EXISTS setup_updated_at;
DROP FUNCTION IF EXISTS trigger_updated_at_no_soft_delete;
DROP FUNCTION IF EXISTS trigger_updated_at;

DROP EXTENSION IF EXISTS pg_trgm;
DROP EXTENSION IF EXISTS pgcrypto;
