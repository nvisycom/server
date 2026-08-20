-- Initial: shared extensions and utility functions every later migration builds
-- on — the updated_at timestamp triggers, secure-token generation, and email
-- validation.

-- pgcrypto: cryptographic primitives (gen_random_bytes for secure tokens).
CREATE EXTENSION IF NOT EXISTS pgcrypto;

-- pg_trgm: trigram text search (fuzzy name/email matching).
CREATE EXTENSION IF NOT EXISTS pg_trgm;

-- Timestamp management function for soft-deletable tables.
--
-- Bumps `updated_at` on change, and on a soft delete syncs `updated_at` with
-- `deleted_at` to satisfy the deleted-after-updated constraint. Once a row is
-- soft-deleted, `updated_at` stays frozen at `deleted_at`: a tombstone's
-- post-deletion system stamps (e.g. a reaper marking its object purged) must not
-- push `updated_at` past `deleted_at`. Requires the table to have a `deleted_at`
-- column; use `trigger_updated_at_no_soft_delete` (via
-- `setup_updated_at_no_soft_delete`) for tables without one.
CREATE OR REPLACE FUNCTION trigger_updated_at()
RETURNS TRIGGER
LANGUAGE plpgsql AS $$
BEGIN
    IF (NEW.deleted_at IS DISTINCT FROM OLD.deleted_at AND NEW.deleted_at IS NOT NULL) THEN
        NEW.updated_at := NEW.deleted_at;
        RETURN NEW;
    END IF;

    -- Already a tombstone (and staying one): keep updated_at pinned to the
    -- deletion time so later mutations of a deleted row don't violate the
    -- deleted-after-updated constraint.
    IF (OLD.deleted_at IS NOT NULL AND NEW.deleted_at IS NOT NULL) THEN
        NEW.updated_at := OLD.updated_at;
        RETURN NEW;
    END IF;

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
    'Automatically updates the updated_at timestamp when a row is modified. For soft deletes, syncs updated_at with deleted_at. Requires a deleted_at column.';

-- Timestamp management function for tables without a `deleted_at` column (e.g.
-- terminal-state records like invites). Bumps `updated_at` on change only; it
-- never references `deleted_at`, so it is safe on non-soft-deletable tables.
CREATE OR REPLACE FUNCTION trigger_updated_at_no_soft_delete()
RETURNS TRIGGER
LANGUAGE plpgsql AS $$
BEGIN
    IF (NEW IS DISTINCT FROM OLD AND NEW.updated_at IS NOT DISTINCT FROM OLD.updated_at) THEN
        NEW.updated_at := CURRENT_TIMESTAMP;
    END IF;
    RETURN NEW;
EXCEPTION
    WHEN OTHERS THEN
        RAISE EXCEPTION 'Error in trigger_updated_at_no_soft_delete for table %: %', TG_TABLE_NAME, SQLERRM;
END;
$$;

COMMENT ON FUNCTION trigger_updated_at_no_soft_delete() IS
    'Automatically updates the updated_at timestamp when a row is modified. For tables without a deleted_at column.';

-- Trigger setup helper for soft-deletable tables (uses `trigger_updated_at`).
CREATE OR REPLACE FUNCTION setup_updated_at(_tbl REGCLASS)
RETURNS VOID
LANGUAGE plpgsql AS $$
BEGIN
    EXECUTE FORMAT(
        'CREATE OR REPLACE TRIGGER trigger_%I_updated_at
         BEFORE UPDATE ON %s
         FOR EACH ROW EXECUTE FUNCTION trigger_updated_at()',
        _tbl, _tbl
    );

    RAISE NOTICE 'Updated_at trigger configured for table: %', _tbl;
EXCEPTION
    WHEN OTHERS THEN
        RAISE EXCEPTION 'Failed to setup updated_at trigger for table %: %', _tbl, SQLERRM;
END;
$$;

COMMENT ON FUNCTION setup_updated_at(_tbl REGCLASS) IS
    'Sets up an updated_at trigger for a soft-deletable table. The table must have updated_at and deleted_at columns.';

-- Trigger setup helper for tables without a `deleted_at` column (uses
-- `trigger_updated_at_no_soft_delete`).
CREATE OR REPLACE FUNCTION setup_updated_at_no_soft_delete(_tbl REGCLASS)
RETURNS VOID
LANGUAGE plpgsql AS $$
BEGIN
    EXECUTE FORMAT(
        'CREATE OR REPLACE TRIGGER trigger_%I_updated_at
         BEFORE UPDATE ON %s
         FOR EACH ROW EXECUTE FUNCTION trigger_updated_at_no_soft_delete()',
        _tbl, _tbl
    );

    RAISE NOTICE 'Updated_at trigger configured for table: %', _tbl;
EXCEPTION
    WHEN OTHERS THEN
        RAISE EXCEPTION 'Failed to setup updated_at trigger for table %: %', _tbl, SQLERRM;
END;
$$;

COMMENT ON FUNCTION setup_updated_at_no_soft_delete(_tbl REGCLASS) IS
    'Sets up an updated_at trigger for a table without a deleted_at column. The table must have an updated_at column.';

-- Security token generation function (URL-safe base64)
CREATE OR REPLACE FUNCTION generate_secure_token(_length INTEGER DEFAULT 32)
RETURNS TEXT
LANGUAGE plpgsql AS $$
BEGIN
    RETURN TRANSLATE(
        REPLACE(ENCODE(gen_random_bytes(_length), 'base64'), '=', ''),
        '+/',
        '-_'
    );
EXCEPTION
    WHEN OTHERS THEN
        RAISE EXCEPTION 'Error generating secure token: %', SQLERRM;
END;
$$;

COMMENT ON FUNCTION generate_secure_token(_length INTEGER) IS
    'Generates a cryptographically secure random token of the specified byte length, URL-safe base64 encoded.';

-- Email validation function
CREATE OR REPLACE FUNCTION is_valid_email(_email TEXT)
RETURNS BOOLEAN
LANGUAGE plpgsql IMMUTABLE AS $$
BEGIN
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
