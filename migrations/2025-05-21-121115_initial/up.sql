-- This migration provides core utility functions for database management and common operations
-- Foundation functions required by all subsequent migrations

-- Enable pgvector extension for vector similarity search
-- Required for embedding storage and semantic search capabilities
CREATE EXTENSION IF NOT EXISTS vector;

-- Timestamp management function
CREATE OR REPLACE FUNCTION trigger_updated_at()
RETURNS TRIGGER
LANGUAGE plpgsql AS $$
BEGIN
    -- Only update if the row has actually changed (excluding updated_at itself)
    IF (NEW IS DISTINCT FROM OLD AND NEW.updated_at IS NOT DISTINCT FROM OLD.updated_at) THEN
        NEW.updated_at := CURRENT_TIMESTAMP;
    END IF;
    RETURN NEW;
EXCEPTION
    WHEN OTHERS THEN
        RAISE EXCEPTION 'Error in trigger_updated_at for table %: %', TG_TABLE_NAME, SQLERRM;
END;
$$;

COMMENT ON FUNCTION trigger_updated_at() IS
    'Automatically updates the updated_at timestamp when a row is modified, but only if the row has actually changed.';

-- Trigger setup helper function
CREATE OR REPLACE FUNCTION setup_updated_at(_tbl REGCLASS)
RETURNS VOID
LANGUAGE plpgsql AS $$
BEGIN
    -- Create or replace the trigger
    EXECUTE FORMAT(
        'CREATE OR REPLACE TRIGGER trigger_%I_updated_at
         BEFORE UPDATE ON %s
         FOR EACH ROW EXECUTE FUNCTION trigger_updated_at()',
        _tbl, _tbl
    );

    -- Log successful setup
    RAISE NOTICE 'Updated_at trigger configured for table: %', _tbl;
EXCEPTION
    WHEN OTHERS THEN
        RAISE EXCEPTION 'Failed to setup updated_at trigger for table %: %', _tbl, SQLERRM;
END;
$$;

COMMENT ON FUNCTION setup_updated_at(_tbl REGCLASS) IS
    'Sets up an updated_at trigger for the specified table. The table must have an updated_at column.';

-- Soft delete management function
CREATE OR REPLACE FUNCTION soft_delete_record(
    _tbl REGCLASS,
    _id_column TEXT,
    _id_value ANYELEMENT
)
RETURNS BOOLEAN
LANGUAGE plpgsql AS $$
DECLARE
    _rows_affected INTEGER;
BEGIN
    -- Perform soft delete by setting deleted_at timestamp
    EXECUTE FORMAT(
        'UPDATE %s SET deleted_at = CURRENT_TIMESTAMP
         WHERE %I = $1 AND deleted_at IS NULL',
        _tbl, _id_column
    ) USING _id_value;

    GET DIAGNOSTICS _rows_affected = ROW_COUNT;

    -- Return true if a row was affected
    RETURN _rows_affected > 0;
EXCEPTION
    WHEN OTHERS THEN
        RAISE EXCEPTION 'Error in soft_delete_record for table % with %=%: %',
            _tbl, _id_column, _id_value, SQLERRM;
END;
$$;

COMMENT ON FUNCTION soft_delete_record(_tbl REGCLASS, _id_column TEXT, _id_value ANYELEMENT) IS
    'Performs a soft delete by setting deleted_at timestamp. Returns true if a record was deleted.';

-- Record restoration function
CREATE OR REPLACE FUNCTION restore_record(
    _tbl REGCLASS,
    _id_column TEXT,
    _id_value ANYELEMENT
)
RETURNS BOOLEAN
LANGUAGE plpgsql AS $$
DECLARE
    _rows_affected INTEGER;
BEGIN
    -- Restore record by clearing deleted_at timestamp
    EXECUTE FORMAT(
        'UPDATE %s SET deleted_at = NULL, updated_at = CURRENT_TIMESTAMP
         WHERE %I = $1 AND deleted_at IS NOT NULL',
        _tbl, _id_column
    ) USING _id_value;

    GET DIAGNOSTICS _rows_affected = ROW_COUNT;

    -- Return true if a row was restored
    RETURN _rows_affected > 0;
EXCEPTION
    WHEN OTHERS THEN
        RAISE EXCEPTION 'Error in restore_record for table % with %=%: %',
            _tbl, _id_column, _id_value, SQLERRM;
END;
$$;

COMMENT ON FUNCTION restore_record(_tbl REGCLASS, _id_column TEXT, _id_value ANYELEMENT) IS
    'Restores a soft-deleted record by clearing the deleted_at timestamp. Returns true if a record was restored.';

-- Expired records cleanup function
CREATE OR REPLACE FUNCTION cleanup_expired_records(
    _tbl REGCLASS,
    _expired_column TEXT DEFAULT 'expired_at'
)
RETURNS INTEGER
LANGUAGE plpgsql AS $$
DECLARE
    _rows_affected INTEGER;
BEGIN
    -- Soft delete expired records
    EXECUTE FORMAT(
        'UPDATE %s SET deleted_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP
         WHERE %I < CURRENT_TIMESTAMP AND deleted_at IS NULL',
        _tbl, _expired_column
    );

    GET DIAGNOSTICS _rows_affected = ROW_COUNT;

    -- Log cleanup activity
    IF _rows_affected > 0 THEN
        RAISE NOTICE 'Cleaned up % expired records from table %', _rows_affected, _tbl;
    END IF;

    RETURN _rows_affected;
EXCEPTION
    WHEN OTHERS THEN
        RAISE EXCEPTION 'Error in cleanup_expired_records for table %: %', _tbl, SQLERRM;
END;
$$;

COMMENT ON FUNCTION cleanup_expired_records(_tbl REGCLASS, _expired_column TEXT) IS
    'Soft deletes expired records based on the specified expiration column. Returns the number of records cleaned up.';

-- Security token generation function
CREATE OR REPLACE FUNCTION generate_secure_token(_length INTEGER DEFAULT 32)
RETURNS TEXT
LANGUAGE plpgsql AS $$
BEGIN
    -- Generate a cryptographically secure random token
    RETURN ENCODE(gen_random_bytes(_length), 'base64');
EXCEPTION
    WHEN OTHERS THEN
        RAISE EXCEPTION 'Error generating secure token: %', SQLERRM;
END;
$$;

COMMENT ON FUNCTION generate_secure_token(_length INTEGER) IS
    'Generates a cryptographically secure random token of the specified byte length, base64 encoded.';

-- Email validation function
CREATE OR REPLACE FUNCTION is_valid_email(_email TEXT)
RETURNS BOOLEAN
LANGUAGE plpgsql IMMUTABLE AS $$
BEGIN
    -- Basic email validation using regex
    RETURN _email ~* '^[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}$'
        AND LENGTH(_email) <= 254
        AND _email NOT LIKE '%@%@%';
EXCEPTION
    WHEN OTHERS THEN
        RETURN FALSE;
END;
$$;

COMMENT ON FUNCTION is_valid_email(_email TEXT) IS
    'Validates email address format using RFC-compliant regex pattern.';
