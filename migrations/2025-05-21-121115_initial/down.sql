-- Drop all utility functions created in the initial migration
-- Drop in reverse order of creation to avoid dependency issues

DROP FUNCTION IF EXISTS is_valid_email(_email TEXT);
DROP FUNCTION IF EXISTS generate_secure_token(_length INTEGER);
DROP FUNCTION IF EXISTS cleanup_expired_records(_tbl REGCLASS, _expired_column TEXT);
DROP FUNCTION IF EXISTS restore_record(_tbl REGCLASS, _id_column TEXT, _id_value ANYELEMENT);
DROP FUNCTION IF EXISTS soft_delete_record(_tbl REGCLASS, _id_column TEXT, _id_value ANYELEMENT);
DROP FUNCTION IF EXISTS setup_updated_at(_tbl REGCLASS);
DROP FUNCTION IF EXISTS trigger_updated_at();
