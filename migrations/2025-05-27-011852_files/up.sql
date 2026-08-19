-- Files: workspace-scoped documents with version chains and content
-- deduplication. Each file records its storage location, a SHA256 content hash,
-- and a data-retention window; versions link to their predecessor via parent_id.

-- Role of a file: drives data-retention scope and whether it is user-facing.
CREATE TYPE FILE_KIND AS ENUM (
    'original',     -- Source document (uploaded or imported)
    'redacted',     -- Redacted output produced by a pipeline
    'audit'         -- Engine analysis blob (not shown in file lists)
);

COMMENT ON TYPE FILE_KIND IS 'The role of a file: original document, redacted output, or audit blob.';

-- Workspace files table: one stored document, with version tracking and dedup.
CREATE TABLE workspace_files (
    -- Primary identifier
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References
    workspace_id            UUID             NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    account_id              UUID             NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    parent_id               UUID             DEFAULT NULL REFERENCES workspace_files (id) ON DELETE SET NULL,

    -- Composite key target for workspace-scoped access and foreign keys.
    CONSTRAINT workspace_files_workspace_id_id_key UNIQUE (workspace_id, id),

    -- Version tracking: parent_id links to the previous version, version_number
    -- tracks the sequence and is set automatically from the parent on insert.
    version_number          INTEGER          NOT NULL DEFAULT 1,
    CONSTRAINT workspace_files_version_number_min CHECK (version_number >= 1),

    -- File metadata
    display_name            TEXT             NOT NULL DEFAULT 'Untitled',
    original_filename       TEXT             NOT NULL DEFAULT 'Untitled',
    file_extension          TEXT             NOT NULL DEFAULT 'txt',
    file_kind               FILE_KIND        NOT NULL DEFAULT 'original',
    CONSTRAINT workspace_files_display_name_length CHECK (length(trim(display_name)) BETWEEN 1 AND 255),
    CONSTRAINT workspace_files_original_filename_length CHECK (length(original_filename) BETWEEN 1 AND 255),
    CONSTRAINT workspace_files_file_extension_format CHECK (file_extension ~ '^[a-zA-Z0-9]{1,20}$'),

    -- Storage and integrity
    file_size_bytes         BIGINT           NOT NULL,
    file_hash_sha256        BYTEA            NOT NULL,
    storage_path            TEXT             NOT NULL,
    storage_bucket          TEXT             NOT NULL,
    CONSTRAINT workspace_files_file_size_min CHECK (file_size_bytes >= 0),
    CONSTRAINT workspace_files_file_hash_sha256_length CHECK (octet_length(file_hash_sha256) = 32),
    CONSTRAINT workspace_files_storage_path_not_empty CHECK (trim(storage_path) <> ''),
    CONSTRAINT workspace_files_storage_bucket_not_empty CHECK (trim(storage_bucket) <> ''),

    -- Configuration
    metadata                JSONB            NOT NULL DEFAULT '{}',
    CONSTRAINT workspace_files_metadata_size CHECK (length(metadata::TEXT) BETWEEN 2 AND 65536),

    -- Lifecycle timestamps
    created_at              TIMESTAMPTZ      NOT NULL DEFAULT current_timestamp,
    updated_at              TIMESTAMPTZ      NOT NULL DEFAULT current_timestamp,
    deleted_at              TIMESTAMPTZ      DEFAULT NULL,
    expires_at              TIMESTAMPTZ      DEFAULT NULL,
    CONSTRAINT workspace_files_updated_after_created CHECK (updated_at >= created_at),
    CONSTRAINT workspace_files_deleted_after_created CHECK (deleted_at IS NULL OR deleted_at >= created_at),
    CONSTRAINT workspace_files_deleted_after_updated CHECK (deleted_at IS NULL OR deleted_at >= updated_at),
    CONSTRAINT workspace_files_expires_after_created CHECK (expires_at IS NULL OR expires_at >= created_at)
);

-- Keep updated_at current on every row modification.
SELECT setup_updated_at('workspace_files');

-- Most recent live files per workspace (the file list).
CREATE INDEX workspace_files_workspace_idx
    ON workspace_files (workspace_id, created_at DESC)
    WHERE deleted_at IS NULL;

-- Most recent live files per account.
CREATE INDEX workspace_files_account_idx
    ON workspace_files (account_id, created_at DESC)
    WHERE deleted_at IS NULL;

-- Deduplication lookup by content hash and size.
CREATE INDEX workspace_files_hash_dedup_idx
    ON workspace_files (file_hash_sha256, file_size_bytes)
    WHERE deleted_at IS NULL;

-- Fuzzy display-name search over live files.
CREATE INDEX workspace_files_display_name_trgm_idx
    ON workspace_files USING gin (display_name gin_trgm_ops)
    WHERE deleted_at IS NULL;

-- Walk a file's version chain, newest version first.
CREATE INDEX workspace_files_version_chain_idx
    ON workspace_files (parent_id, version_number DESC)
    WHERE parent_id IS NOT NULL AND deleted_at IS NULL;

-- Data-retention sweep: live files whose retention window has elapsed.
CREATE INDEX workspace_files_expiry_idx
    ON workspace_files (expires_at)
    WHERE expires_at IS NOT NULL AND deleted_at IS NULL;

-- Sets version_number on insert: one past the parent's version, or 1 with no parent.
CREATE OR REPLACE FUNCTION set_workspace_file_version_number()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.parent_id IS NOT NULL THEN
        SELECT version_number + 1 INTO NEW.version_number
        FROM workspace_files
        WHERE id = NEW.parent_id;
    ELSE
        NEW.version_number := 1;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Derives version_number from the parent before each row is inserted.
CREATE TRIGGER workspace_files_set_version_trigger
    BEFORE INSERT ON workspace_files
    FOR EACH ROW
    EXECUTE FUNCTION set_workspace_file_version_number();

COMMENT ON FUNCTION set_workspace_file_version_number() IS 'Automatically sets version_number based on parent file version.';

COMMENT ON TABLE workspace_files IS 'Files stored in the system with version tracking and deduplication.';
COMMENT ON COLUMN workspace_files.id IS 'Unique file identifier';
COMMENT ON COLUMN workspace_files.workspace_id IS 'Parent workspace reference';
COMMENT ON COLUMN workspace_files.account_id IS 'Uploading/creating account reference';
COMMENT ON COLUMN workspace_files.parent_id IS 'Parent file reference for version chains';
COMMENT ON COLUMN workspace_files.version_number IS 'Version number (1 for original, increments via parent_id chain)';
COMMENT ON COLUMN workspace_files.display_name IS 'Display name (1-255 chars)';
COMMENT ON COLUMN workspace_files.original_filename IS 'Original upload filename (1-255 chars)';
COMMENT ON COLUMN workspace_files.file_extension IS 'File extension (1-20 alphanumeric); Content-Type is derived from it';
COMMENT ON COLUMN workspace_files.file_kind IS 'Role of the file (original, redacted, audit); drives retention scope and list visibility';
COMMENT ON COLUMN workspace_files.file_size_bytes IS 'File size in bytes';
COMMENT ON COLUMN workspace_files.file_hash_sha256 IS 'SHA256 content hash';
COMMENT ON COLUMN workspace_files.storage_path IS 'Storage system path';
COMMENT ON COLUMN workspace_files.storage_bucket IS 'Storage bucket/container';
COMMENT ON COLUMN workspace_files.metadata IS 'Extended metadata (JSON)';
COMMENT ON COLUMN workspace_files.created_at IS 'Upload timestamp';
COMMENT ON COLUMN workspace_files.updated_at IS 'Last modification timestamp';
COMMENT ON COLUMN workspace_files.deleted_at IS 'Soft deletion timestamp';
COMMENT ON COLUMN workspace_files.expires_at IS 'Data-retention expiry (NULL = keep indefinitely)';

-- Groups live files by hash and size to surface duplicates, optionally within one workspace.
CREATE OR REPLACE FUNCTION find_duplicate_workspace_files(_workspace_id UUID DEFAULT NULL)
RETURNS TABLE (
    file_hash TEXT,
    file_size BIGINT,
    duplicate_count BIGINT,
    file_ids UUID[]
)
LANGUAGE plpgsql AS $$
BEGIN
    RETURN QUERY
    SELECT
        ENCODE(f.file_hash_sha256, 'hex'),
        f.file_size_bytes,
        COUNT(*),
        ARRAY_AGG(f.id)
    FROM workspace_files f
    WHERE (_workspace_id IS NULL OR f.workspace_id = _workspace_id)
        AND f.deleted_at IS NULL
    GROUP BY f.file_hash_sha256, f.file_size_bytes
    HAVING COUNT(*) > 1
    ORDER BY COUNT(*) DESC;
END;
$$;

COMMENT ON FUNCTION find_duplicate_workspace_files(UUID) IS 'Finds duplicate workspace files by hash and size. Optionally scoped to a specific workspace.';
