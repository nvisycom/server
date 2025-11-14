-- This migration creates tables for documents, files, processing pipeline, and security features

-- Create document status enum
CREATE TYPE DOCUMENT_STATUS AS ENUM (
    'draft',        -- Document is being created/edited
    'processing',   -- Document is being processed
    'ready',        -- Document is ready for use
    'archived',     -- Document is archived but accessible
    'locked',       -- Document is locked for editing
    'error'         -- Document processing failed
);

COMMENT ON TYPE DOCUMENT_STATUS IS
    'Document lifecycle status for tracking processing and availability.';

-- Create file processing status enum
CREATE TYPE PROCESSING_STATUS AS ENUM (
    'pending',      -- File is queued for processing
    'processing',   -- File is currently being processed
    'completed',    -- Processing completed successfully
    'failed',       -- Processing failed
    'canceled',     -- Processing was canceled
    'skipped',      -- Processing was skipped
    'retry'         -- Processing is queued for retry
);

COMMENT ON TYPE PROCESSING_STATUS IS
    'File processing pipeline status for tracking processing workflows.';

-- Create processing requirements enum
CREATE TYPE REQUIRE_MODE AS ENUM (
    'text',         -- Plain text content ready for analysis
    'ocr',          -- Requires optical character recognition
    'transcribe',   -- Requires audio/video transcription
    'mixed'         -- May require multiple processing modes
);

COMMENT ON TYPE REQUIRE_MODE IS
    'Processing requirements for input files based on content type.';

-- Create virus scan status enum
CREATE TYPE VIRUS_SCAN_STATUS AS ENUM (
    'pending',      -- Scan is pending
    'clean',        -- No virus detected
    'infected',     -- Virus detected
    'suspicious',   -- Suspicious activity detected
    'unknown'       -- Unable to determine status
);

COMMENT ON TYPE VIRUS_SCAN_STATUS IS
    'Security scan results for uploaded files.';

-- Create documents table - Document containers/folders
CREATE TABLE documents (
    -- Primary identifiers
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References
    project_id      UUID             NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    account_id      UUID             NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,

    -- Core attributes
    display_name    TEXT             NOT NULL DEFAULT 'Untitled',
    description     TEXT             NOT NULL DEFAULT '',
    tags            TEXT[]           NOT NULL DEFAULT '{}',
    status          DOCUMENT_STATUS  NOT NULL DEFAULT 'draft',

    CONSTRAINT documents_display_name_length CHECK (length(trim(display_name)) BETWEEN 1 AND 255),
    CONSTRAINT documents_description_length_max CHECK (length(description) <= 2048),
    CONSTRAINT documents_tags_count_max CHECK (array_length(tags, 1) IS NULL OR array_length(tags, 1) <= 32),

    -- Configuration
    metadata        JSONB            NOT NULL DEFAULT '{}',
    settings        JSONB            NOT NULL DEFAULT '{}',

    CONSTRAINT documents_metadata_size CHECK (length(metadata::TEXT) BETWEEN 2 AND 16384),
    CONSTRAINT documents_settings_size CHECK (length(settings::TEXT) BETWEEN 2 AND 8192),

    -- Lifecycle timestamps
    created_at      TIMESTAMPTZ      NOT NULL DEFAULT current_timestamp,
    updated_at      TIMESTAMPTZ      NOT NULL DEFAULT current_timestamp,
    deleted_at      TIMESTAMPTZ      DEFAULT NULL,

    CONSTRAINT documents_updated_after_created CHECK (updated_at >= created_at),
    CONSTRAINT documents_deleted_after_created CHECK (deleted_at IS NULL OR deleted_at >= created_at),
    CONSTRAINT documents_deleted_after_updated CHECK (deleted_at IS NULL OR deleted_at >= updated_at)
);

-- Set up automatic updated_at trigger
SELECT setup_updated_at('documents');

-- Create indexes for documents
CREATE INDEX documents_project_status_idx
    ON documents (project_id, status, created_at DESC)
    WHERE deleted_at IS NULL;

CREATE INDEX documents_account_recent_idx
    ON documents (account_id, updated_at DESC)
    WHERE deleted_at IS NULL;

CREATE INDEX documents_tags_search_idx
    ON documents USING gin (tags)
    WHERE array_length(tags, 1) > 0 AND deleted_at IS NULL;

CREATE INDEX documents_metadata_search_idx
    ON documents USING gin (metadata)
    WHERE deleted_at IS NULL;

-- Add table and column comments
COMMENT ON TABLE documents IS
    'Document containers for organizing and managing file collections with metadata and settings.';

COMMENT ON COLUMN documents.id IS 'Unique document identifier';
COMMENT ON COLUMN documents.project_id IS 'Parent project reference';
COMMENT ON COLUMN documents.account_id IS 'Creating account reference';
COMMENT ON COLUMN documents.display_name IS 'Human-readable document name (1-255 chars)';
COMMENT ON COLUMN documents.description IS 'Document description (up to 2048 chars)';
COMMENT ON COLUMN documents.tags IS 'Classification tags (max 32)';
COMMENT ON COLUMN documents.status IS 'Current document lifecycle status';
COMMENT ON COLUMN documents.metadata IS 'Extended metadata (JSON, 2B-16KB)';
COMMENT ON COLUMN documents.settings IS 'Document configuration (JSON, 2B-8KB)';
COMMENT ON COLUMN documents.created_at IS 'Creation timestamp';
COMMENT ON COLUMN documents.updated_at IS 'Last modification timestamp';
COMMENT ON COLUMN documents.deleted_at IS 'Soft deletion timestamp';

-- Create document files table - Source files for processing
CREATE TABLE document_files (
    -- Primary identifiers
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References
    document_id         UUID             NOT NULL REFERENCES documents (id) ON DELETE CASCADE,
    account_id          UUID             NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,

    -- File metadata
    display_name        TEXT             NOT NULL DEFAULT 'Untitled',
    original_filename   TEXT             NOT NULL DEFAULT 'Untitled',
    file_extension      TEXT             NOT NULL DEFAULT 'txt',

    CONSTRAINT document_files_display_name_length CHECK (length(trim(display_name)) BETWEEN 1 AND 255),
    CONSTRAINT document_files_original_filename_length CHECK (length(original_filename) BETWEEN 1 AND 255),
    CONSTRAINT document_files_file_extension_format CHECK (file_extension ~ '^[a-zA-Z0-9]{1,20}$'),

    -- Processing configuration
    require_mode        REQUIRE_MODE     NOT NULL DEFAULT 'text',
    processing_priority INTEGER          NOT NULL DEFAULT 5,
    processing_status   PROCESSING_STATUS NOT NULL DEFAULT 'pending',
    virus_scan_status   VIRUS_SCAN_STATUS NOT NULL DEFAULT 'pending',

    CONSTRAINT document_files_processing_priority_range CHECK (processing_priority BETWEEN 1 AND 10),

    -- Storage and integrity
    file_size_bytes     BIGINT           NOT NULL DEFAULT 0,
    file_hash_sha256    BYTEA            NOT NULL,
    storage_path        TEXT             NOT NULL,
    storage_bucket      TEXT             NOT NULL DEFAULT '',

    CONSTRAINT document_files_file_size_min CHECK (file_size_bytes >= 0),
    CONSTRAINT document_files_file_hash_sha256_length CHECK (octet_length(file_hash_sha256) = 32),
    CONSTRAINT document_files_storage_path_not_empty CHECK (trim(storage_path) <> ''),
    CONSTRAINT document_files_storage_bucket_not_empty CHECK (trim(storage_bucket) <> ''),

    -- Configuration and retention policy
    metadata            JSONB            NOT NULL DEFAULT '{}',
    keep_for_sec        INTEGER          NOT NULL DEFAULT 31536000,
    auto_delete_at      TIMESTAMPTZ      DEFAULT NULL,

    CONSTRAINT document_files_metadata_size CHECK (length(metadata::TEXT) BETWEEN 2 AND 8192),
    CONSTRAINT document_files_retention_period CHECK (keep_for_sec BETWEEN 3600 AND 157680000),

    -- Lifecycle timestamps
    created_at          TIMESTAMPTZ      NOT NULL DEFAULT current_timestamp,
    updated_at          TIMESTAMPTZ      NOT NULL DEFAULT current_timestamp,
    deleted_at          TIMESTAMPTZ      DEFAULT NULL,

    CONSTRAINT document_files_updated_after_created CHECK (updated_at >= created_at),
    CONSTRAINT document_files_deleted_after_created CHECK (deleted_at IS NULL OR deleted_at >= created_at),
    CONSTRAINT document_files_deleted_after_updated CHECK (deleted_at IS NULL OR deleted_at >= updated_at),
    CONSTRAINT document_files_auto_delete_after_created CHECK (auto_delete_at IS NULL OR auto_delete_at > created_at)
);

-- Create auto-delete trigger function
CREATE OR REPLACE FUNCTION set_document_file_auto_delete()
RETURNS TRIGGER AS $$
BEGIN
    NEW.auto_delete_at := NEW.created_at + (NEW.keep_for_sec || ' seconds')::INTERVAL;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Create auto-delete trigger
CREATE TRIGGER document_files_auto_delete_trigger
    BEFORE INSERT OR UPDATE OF keep_for_sec ON document_files
    FOR EACH ROW EXECUTE FUNCTION set_document_file_auto_delete();

-- Set up automatic updated_at trigger
SELECT setup_updated_at('document_files');

-- Create indexes for document files
CREATE INDEX document_files_document_status_idx
    ON document_files (document_id, processing_status, created_at DESC)
    WHERE deleted_at IS NULL;

CREATE INDEX document_files_processing_queue_idx
    ON document_files (processing_status, processing_priority DESC, created_at ASC)
    WHERE processing_status IN ('pending', 'retry') AND deleted_at IS NULL;

CREATE INDEX document_files_hash_dedup_idx
    ON document_files (file_hash_sha256, file_size_bytes)
    WHERE deleted_at IS NULL;

CREATE INDEX document_files_cleanup_idx
    ON document_files (auto_delete_at)
    WHERE auto_delete_at IS NOT NULL AND deleted_at IS NULL;

-- Add table and column comments
COMMENT ON TABLE document_files IS
    'Source files for document processing with pipeline management and security scanning.';

COMMENT ON COLUMN document_files.id IS 'Unique file identifier';
COMMENT ON COLUMN document_files.document_id IS 'Parent document reference';
COMMENT ON COLUMN document_files.account_id IS 'Uploading account reference';
COMMENT ON COLUMN document_files.display_name IS 'Display name (1-255 chars)';
COMMENT ON COLUMN document_files.original_filename IS 'Original upload filename (1-255 chars)';
COMMENT ON COLUMN document_files.file_extension IS 'File extension (1-20 alphanumeric)';
COMMENT ON COLUMN document_files.require_mode IS 'Processing mode required';
COMMENT ON COLUMN document_files.processing_priority IS 'Priority 1-10 (1=highest)';
COMMENT ON COLUMN document_files.processing_status IS 'Current processing status';
COMMENT ON COLUMN document_files.virus_scan_status IS 'Security scan result';
COMMENT ON COLUMN document_files.file_size_bytes IS 'File size in bytes';
COMMENT ON COLUMN document_files.file_hash_sha256 IS 'SHA256 content hash';
COMMENT ON COLUMN document_files.storage_path IS 'Storage system path';
COMMENT ON COLUMN document_files.storage_bucket IS 'Storage bucket/container';
COMMENT ON COLUMN document_files.metadata IS 'Extended metadata (JSON, 2B-8KB)';
COMMENT ON COLUMN document_files.keep_for_sec IS 'Retention period (1h-5y)';
COMMENT ON COLUMN document_files.auto_delete_at IS 'Automatic deletion timestamp';
COMMENT ON COLUMN document_files.created_at IS 'Upload timestamp';
COMMENT ON COLUMN document_files.updated_at IS 'Last modification timestamp';
COMMENT ON COLUMN document_files.deleted_at IS 'Soft deletion timestamp';

-- Create document versions table - Processed outputs
CREATE TABLE document_versions (
    -- Primary identifiers
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References
    document_id         UUID             NOT NULL REFERENCES documents (id) ON DELETE CASCADE,
    account_id          UUID             NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    version_number      INTEGER          NOT NULL,

    CONSTRAINT document_versions_version_number_min CHECK (version_number >= 1),

    -- File metadata
    display_name        TEXT             NOT NULL DEFAULT 'Untitled',
    file_extension      TEXT             NOT NULL DEFAULT 'txt',

    CONSTRAINT document_versions_display_name_length CHECK (length(trim(display_name)) BETWEEN 1 AND 255),
    CONSTRAINT document_versions_file_extension_format CHECK (file_extension ~ '^[a-zA-Z0-9]{1,20}$'),

    -- Processing metrics
    processing_credits  INTEGER          NOT NULL DEFAULT 0,
    processing_duration INTEGER          NOT NULL DEFAULT 0,
    api_calls_made      INTEGER          NOT NULL DEFAULT 0,

    CONSTRAINT document_versions_processing_credits_min CHECK (processing_credits >= 0),
    CONSTRAINT document_versions_processing_duration_min CHECK (processing_duration >= 0),
    CONSTRAINT document_versions_api_calls_min CHECK (api_calls_made >= 0),

    -- Storage and integrity
    file_size_bytes     BIGINT           NOT NULL DEFAULT 0,
    file_hash_sha256    BYTEA            NOT NULL,
    storage_path        TEXT             NOT NULL,
    storage_bucket      TEXT             NOT NULL DEFAULT '',

    CONSTRAINT document_versions_file_size_min CHECK (file_size_bytes >= 0),
    CONSTRAINT document_versions_file_hash_sha256_length CHECK (octet_length(file_hash_sha256) = 32),
    CONSTRAINT document_versions_storage_path_not_empty CHECK (trim(storage_path) <> ''),
    CONSTRAINT document_versions_storage_bucket_not_empty CHECK (trim(storage_bucket) <> ''),

    -- Content, metadata and retention policy
    results             JSONB            NOT NULL DEFAULT '{}',
    metadata            JSONB            NOT NULL DEFAULT '{}',
    keep_for_sec        INTEGER          NOT NULL DEFAULT 31536000,
    auto_delete_at      TIMESTAMPTZ      DEFAULT NULL,

    CONSTRAINT document_versions_retention_period CHECK (keep_for_sec BETWEEN 3600 AND 157680000),
    CONSTRAINT document_versions_results_size CHECK (length(results::TEXT) BETWEEN 2 AND 65536),
    CONSTRAINT document_versions_metadata_size CHECK (length(metadata::TEXT) BETWEEN 2 AND 16384),

    -- Lifecycle timestamps
    created_at          TIMESTAMPTZ      NOT NULL DEFAULT current_timestamp,
    updated_at          TIMESTAMPTZ      NOT NULL DEFAULT current_timestamp,
    deleted_at          TIMESTAMPTZ      DEFAULT NULL,

    CONSTRAINT document_versions_updated_after_created CHECK (updated_at >= created_at),
    CONSTRAINT document_versions_deleted_after_created CHECK (deleted_at IS NULL OR deleted_at >= created_at),
    CONSTRAINT document_versions_deleted_after_updated CHECK (deleted_at IS NULL OR deleted_at >= updated_at),
    CONSTRAINT document_versions_auto_delete_after_created CHECK (auto_delete_at IS NULL OR auto_delete_at > created_at),

    -- Business logic constraints
    CONSTRAINT document_versions_unique_per_document UNIQUE (document_id, version_number)
);

-- Create auto-delete trigger function
CREATE OR REPLACE FUNCTION set_document_version_auto_delete()
RETURNS TRIGGER AS $$
BEGIN
    NEW.auto_delete_at := NEW.created_at + (NEW.keep_for_sec || ' seconds')::INTERVAL;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Create auto-delete trigger
CREATE TRIGGER document_versions_auto_delete_trigger
    BEFORE INSERT OR UPDATE OF keep_for_sec ON document_versions
    FOR EACH ROW EXECUTE FUNCTION set_document_version_auto_delete();

-- Set up automatic updated_at trigger
SELECT setup_updated_at('document_versions');

-- Create indexes for document versions
CREATE INDEX document_versions_document_idx
    ON document_versions (document_id, version_number DESC)
    WHERE deleted_at IS NULL;

CREATE INDEX document_versions_account_recent_idx
    ON document_versions (account_id, created_at DESC)
    WHERE deleted_at IS NULL;

CREATE INDEX document_versions_hash_dedup_idx
    ON document_versions (file_hash_sha256, file_size_bytes)
    WHERE deleted_at IS NULL;

CREATE INDEX document_versions_cleanup_idx
    ON document_versions (auto_delete_at)
    WHERE auto_delete_at IS NOT NULL AND deleted_at IS NULL;

CREATE INDEX document_versions_processing_cost_idx
    ON document_versions (processing_credits, created_at DESC)
    WHERE processing_credits > 0 AND deleted_at IS NULL;

-- Add table and column comments
COMMENT ON TABLE document_versions IS
    'Processed document versions with processing metrics and analysis results.';

COMMENT ON COLUMN document_versions.id IS 'Unique version identifier';
COMMENT ON COLUMN document_versions.document_id IS 'Parent document reference';
COMMENT ON COLUMN document_versions.account_id IS 'Processing initiator reference';
COMMENT ON COLUMN document_versions.version_number IS 'Sequential version number (starts at 1)';
COMMENT ON COLUMN document_versions.display_name IS 'Version display name (1-255 chars)';
COMMENT ON COLUMN document_versions.file_extension IS 'Output file extension';
COMMENT ON COLUMN document_versions.processing_credits IS 'Processing credits consumed';
COMMENT ON COLUMN document_versions.processing_duration IS 'Processing time (milliseconds)';
COMMENT ON COLUMN document_versions.api_calls_made IS 'External API calls count';
COMMENT ON COLUMN document_versions.file_size_bytes IS 'Output file size in bytes';
COMMENT ON COLUMN document_versions.file_hash_sha256 IS 'SHA256 content hash';
COMMENT ON COLUMN document_versions.storage_path IS 'Storage system path';
COMMENT ON COLUMN document_versions.storage_bucket IS 'Storage bucket/container';
COMMENT ON COLUMN document_versions.results IS 'Processing results (JSON, 2B-64KB)';
COMMENT ON COLUMN document_versions.metadata IS 'Version metadata (JSON, 2B-16KB)';
COMMENT ON COLUMN document_versions.keep_for_sec IS 'Retention period (1h-5y)';
COMMENT ON COLUMN document_versions.auto_delete_at IS 'Automatic deletion timestamp';
COMMENT ON COLUMN document_versions.created_at IS 'Processing completion timestamp';
COMMENT ON COLUMN document_versions.updated_at IS 'Last modification timestamp';
COMMENT ON COLUMN document_versions.deleted_at IS 'Soft deletion timestamp';

-- Create document processing summary view
CREATE VIEW document_processing_summary AS
SELECT
    d.id,
    d.display_name,
    d.status,
    d.project_id,
    COUNT(df.id) FILTER (WHERE df.deleted_at IS NULL) AS input_files_count,
    COUNT(dv.id) FILTER (WHERE dv.deleted_at IS NULL) AS output_versions_count,
    COALESCE(SUM(dv.processing_credits), 0) AS total_credits_used,
    MAX(dv.created_at) AS latest_version_at,
    d.created_at,
    d.updated_at
FROM documents d
    LEFT JOIN document_files df ON d.id = df.document_id
    LEFT JOIN document_versions dv ON d.id = dv.document_id
WHERE d.deleted_at IS NULL
GROUP BY d.id, d.display_name, d.status, d.project_id, d.created_at, d.updated_at;

COMMENT ON VIEW document_processing_summary IS
    'Overview of document processing status, metrics, and costs.';

-- Create processing queue view
CREATE VIEW processing_queue AS
SELECT
    df.id,
    df.document_id,
    d.display_name AS document_name,
    d.project_id,
    df.display_name AS file_name,
    df.require_mode,
    df.processing_priority,
    df.processing_status,
    df.file_size_bytes,
    df.created_at,
    EXTRACT(EPOCH FROM (CURRENT_TIMESTAMP - df.created_at)) AS queue_time_seconds
FROM document_files df
    JOIN documents d ON df.document_id = d.id
WHERE df.processing_status IN ('pending', 'retry', 'processing')
    AND df.deleted_at IS NULL
    AND d.deleted_at IS NULL
ORDER BY df.processing_priority DESC, df.created_at ASC;

COMMENT ON VIEW processing_queue IS
    'Files queued for processing, ordered by priority and age.';

-- Create document version function
CREATE OR REPLACE FUNCTION create_document_version(
    _document_id UUID,
    _account_id UUID
) RETURNS UUID
LANGUAGE plpgsql AS $$
DECLARE
    _version_id UUID;
    _version_number INTEGER;
BEGIN
    -- Get next version number
    SELECT COALESCE(MAX(version_number), 0) + 1
    INTO _version_number
    FROM document_versions
    WHERE document_id = _document_id
        AND deleted_at IS NULL;

    -- Create version record
    INSERT INTO document_versions (document_id, account_id, version_number)
    VALUES (_document_id, _account_id, _version_number)
    RETURNING id INTO _version_id;

    RETURN _version_id;
END;
$$;

COMMENT ON FUNCTION create_document_version(UUID, UUID) IS
    'Creates a new version of a document with auto-incrementing version number.';

-- Create cleanup function
CREATE OR REPLACE FUNCTION cleanup_expired_documents()
RETURNS TABLE (
    files_cleaned INTEGER,
    versions_cleaned INTEGER,
    storage_freed_mb DECIMAL(10,2)
)
LANGUAGE plpgsql AS $$
DECLARE
    file_count INTEGER := 0;
    version_count INTEGER := 0;
    storage_freed DECIMAL(10,2) := 0.00;
BEGIN
    -- Clean up expired files
    WITH deleted_files AS (
        UPDATE document_files
        SET deleted_at = CURRENT_TIMESTAMP,
            updated_at = CURRENT_TIMESTAMP
        WHERE (
            auto_delete_at < CURRENT_TIMESTAMP
            OR EXTRACT(EPOCH FROM (CURRENT_TIMESTAMP - created_at)) > keep_for_sec
        )
        AND deleted_at IS NULL
        RETURNING file_size_bytes
    )
    SELECT COUNT(*), ROUND(COALESCE(SUM(file_size_bytes), 0) / 1048576.0, 2)
    INTO file_count, storage_freed
    FROM deleted_files;

    -- Clean up expired versions
    WITH deleted_versions AS (
        UPDATE document_versions
        SET deleted_at = CURRENT_TIMESTAMP,
            updated_at = CURRENT_TIMESTAMP
        WHERE (
            auto_delete_at < CURRENT_TIMESTAMP
            OR EXTRACT(EPOCH FROM (CURRENT_TIMESTAMP - created_at)) > keep_for_sec
        )
        AND deleted_at IS NULL
        RETURNING file_size_bytes
    )
    SELECT version_count + COUNT(*),
           storage_freed + ROUND(COALESCE(SUM(file_size_bytes), 0) / 1048576.0, 2)
    INTO version_count, storage_freed
    FROM deleted_versions;

    RETURN QUERY SELECT file_count, version_count, storage_freed;
END;
$$;

COMMENT ON FUNCTION cleanup_expired_documents() IS
    'Cleans up expired files and versions based on retention policies. Returns cleanup statistics.';

-- Create duplicate detection function
CREATE OR REPLACE FUNCTION find_duplicate_files(_document_id UUID DEFAULT NULL)
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
        ENCODE(df.file_hash_sha256, 'hex'),
        df.file_size_bytes,
        COUNT(*),
        ARRAY_AGG(df.id)
    FROM document_files df
    WHERE (_document_id IS NULL OR df.document_id = _document_id)
        AND df.deleted_at IS NULL
    GROUP BY df.file_hash_sha256, df.file_size_bytes
    HAVING COUNT(*) > 1
    ORDER BY COUNT(*) DESC;
END;
$$;

COMMENT ON FUNCTION find_duplicate_files(UUID) IS
    'Finds duplicate files by hash and size. Optionally scoped to a specific document.';

-- Create document comments table - User discussions and annotations
CREATE TABLE document_comments (
    -- Primary identifiers
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References (exactly one target must be set)
    document_id         UUID             DEFAULT NULL REFERENCES documents (id) ON DELETE CASCADE,
    document_file_id    UUID             DEFAULT NULL REFERENCES document_files (id) ON DELETE CASCADE,
    document_version_id UUID             DEFAULT NULL REFERENCES document_versions (id) ON DELETE CASCADE,
    account_id          UUID             NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,

    -- Thread references
    parent_comment_id   UUID             DEFAULT NULL REFERENCES document_comments (id) ON DELETE CASCADE,
    reply_to_account_id UUID             DEFAULT NULL REFERENCES accounts (id) ON DELETE SET NULL,

    -- Comment content
    content             TEXT             NOT NULL,

    CONSTRAINT document_comments_content_length CHECK (length(trim(content)) BETWEEN 1 AND 10000),
    CONSTRAINT document_comments_one_target CHECK (
        (document_id IS NOT NULL)::INTEGER +
        (document_file_id IS NOT NULL)::INTEGER +
        (document_version_id IS NOT NULL)::INTEGER = 1
    ),

    -- Metadata
    metadata            JSONB            NOT NULL DEFAULT '{}',

    CONSTRAINT document_comments_metadata_size CHECK (length(metadata::TEXT) BETWEEN 2 AND 4096),

    -- Lifecycle timestamps
    created_at          TIMESTAMPTZ      NOT NULL DEFAULT current_timestamp,
    updated_at          TIMESTAMPTZ      NOT NULL DEFAULT current_timestamp,
    deleted_at          TIMESTAMPTZ      DEFAULT NULL,

    CONSTRAINT document_comments_updated_after_created CHECK (updated_at >= created_at),
    CONSTRAINT document_comments_deleted_after_created CHECK (deleted_at IS NULL OR deleted_at >= created_at),
    CONSTRAINT document_comments_deleted_after_updated CHECK (deleted_at IS NULL OR deleted_at >= updated_at)
);

-- Set up automatic updated_at trigger
SELECT setup_updated_at('document_comments');

-- Create indexes for document comments
CREATE INDEX document_comments_document_idx
    ON document_comments (document_id, created_at DESC)
    WHERE document_id IS NOT NULL AND deleted_at IS NULL;

CREATE INDEX document_comments_file_idx
    ON document_comments (document_file_id, created_at DESC)
    WHERE document_file_id IS NOT NULL AND deleted_at IS NULL;

CREATE INDEX document_comments_version_idx
    ON document_comments (document_version_id, created_at DESC)
    WHERE document_version_id IS NOT NULL AND deleted_at IS NULL;

CREATE INDEX document_comments_account_idx
    ON document_comments (account_id, created_at DESC)
    WHERE deleted_at IS NULL;

CREATE INDEX document_comments_thread_idx
    ON document_comments (parent_comment_id, created_at ASC)
    WHERE parent_comment_id IS NOT NULL AND deleted_at IS NULL;

CREATE INDEX document_comments_reply_to_idx
    ON document_comments (reply_to_account_id, created_at DESC)
    WHERE reply_to_account_id IS NOT NULL AND deleted_at IS NULL;

CREATE INDEX document_comments_metadata_idx
    ON document_comments USING gin (metadata)
    WHERE deleted_at IS NULL;

-- Add table and column comments
COMMENT ON TABLE document_comments IS
    'User comments and discussions about documents, files, or versions, supporting threaded conversations and @mentions.';

COMMENT ON COLUMN document_comments.id IS 'Unique comment identifier';
COMMENT ON COLUMN document_comments.document_id IS 'Parent document reference (mutually exclusive with file/version)';
COMMENT ON COLUMN document_comments.document_file_id IS 'Parent document file reference (mutually exclusive with document/version)';
COMMENT ON COLUMN document_comments.document_version_id IS 'Parent document version reference (mutually exclusive with document/file)';
COMMENT ON COLUMN document_comments.account_id IS 'Comment author reference';
COMMENT ON COLUMN document_comments.parent_comment_id IS 'Parent comment for threaded replies (NULL for top-level)';
COMMENT ON COLUMN document_comments.reply_to_account_id IS 'Account being replied to (@mention)';
COMMENT ON COLUMN document_comments.content IS 'Comment text content (1-10000 chars)';
COMMENT ON COLUMN document_comments.metadata IS 'Extended metadata (JSON, 2B-4KB)';
COMMENT ON COLUMN document_comments.created_at IS 'Comment creation timestamp';
COMMENT ON COLUMN document_comments.updated_at IS 'Last edit timestamp';
COMMENT ON COLUMN document_comments.deleted_at IS 'Soft deletion timestamp';
