-- This migration creates tables for documents, files, processing pipeline, and security features

-- Create document status enum
CREATE TYPE DOCUMENT_STATUS AS ENUM (
    'draft',        -- Document is being created/edited
    'processing',   -- Document is being processed
    'ready',        -- Document is ready for use
    'archived'      -- Document is archived but accessible
);

COMMENT ON TYPE DOCUMENT_STATUS IS
    'Document lifecycle status for tracking processing and availability.';

-- Create documents table - Document containers/folders
CREATE TABLE documents (
    -- Primary identifiers
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References
    project_id      UUID             NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    account_id      UUID             NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,

    -- Core attributes
    display_name    TEXT             NOT NULL DEFAULT 'Untitled',
    description     TEXT                      DEFAULT NULL,
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

-- Create file processing status enum
CREATE TYPE PROCESSING_STATUS AS ENUM (
    'pending',      -- File is queued for processing
    'processing',   -- File is currently being processed
    'completed',    -- Processing completed successfully
    'failed',       -- Processing failed
    'canceled',     -- Processing was canceled
    'skipped'       -- Processing was skipped
);

COMMENT ON TYPE PROCESSING_STATUS IS
    'File processing pipeline status for tracking processing workflows.';

-- Create processing requirements enum
CREATE TYPE REQUIRE_MODE AS ENUM (
    'none',         -- No special processing required
    'optical',      -- Requires OCR to extract text from images
    'language',     -- Requires VLM for advanced content understanding
    'both'          -- Requires both OCR and VLM processing
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

-- Create content segmentation enum
CREATE TYPE CONTENT_SEGMENTATION AS ENUM (
    'none',         -- No segmentation applied
    'semantic',     -- Semantic-based segmentation
    'fixed'         -- Fixed-size segmentation
);

COMMENT ON TYPE CONTENT_SEGMENTATION IS
    'Content segmentation strategy for document processing.';

-- Create document files table - Source files for processing
CREATE TABLE document_files (
    -- Primary identifiers
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References
    project_id              UUID             NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    document_id             UUID             DEFAULT NULL REFERENCES documents (id) ON DELETE CASCADE,
    account_id              UUID             NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    parent_id               UUID             DEFAULT NULL REFERENCES document_files (id) ON DELETE SET NULL,

    -- File metadata
    display_name            TEXT             NOT NULL DEFAULT 'Untitled',
    original_filename       TEXT             NOT NULL DEFAULT 'Untitled',
    file_extension          TEXT             NOT NULL DEFAULT 'txt',
    tags                    TEXT[]           NOT NULL DEFAULT '{}',

    CONSTRAINT document_files_display_name_length CHECK (length(trim(display_name)) BETWEEN 1 AND 255),
    CONSTRAINT document_files_original_filename_length CHECK (length(original_filename) BETWEEN 1 AND 255),
    CONSTRAINT document_files_file_extension_format CHECK (file_extension ~ '^[a-zA-Z0-9]{1,20}$'),
    CONSTRAINT document_files_tags_count_max CHECK (array_length(tags, 1) IS NULL OR array_length(tags, 1) <= 32),

    -- Processing configuration
    require_mode            REQUIRE_MODE     NOT NULL DEFAULT 'none',
    processing_priority     INTEGER          NOT NULL DEFAULT 5,
    processing_status       PROCESSING_STATUS NOT NULL DEFAULT 'pending',
    virus_scan_status       VIRUS_SCAN_STATUS NOT NULL DEFAULT 'pending',

    CONSTRAINT document_files_processing_priority_range CHECK (processing_priority BETWEEN 1 AND 10),

    -- Knowledge extraction configuration
    is_indexed              BOOLEAN          NOT NULL DEFAULT FALSE,
    content_segmentation    CONTENT_SEGMENTATION NOT NULL DEFAULT 'semantic',
    visual_support          BOOLEAN          NOT NULL DEFAULT FALSE,

    -- Storage and integrity
    file_size_bytes         BIGINT           NOT NULL DEFAULT 0,
    file_hash_sha256        BYTEA            NOT NULL,
    storage_path            TEXT             NOT NULL,
    storage_bucket          TEXT             NOT NULL DEFAULT '',

    CONSTRAINT document_files_file_size_min CHECK (file_size_bytes >= 0),
    CONSTRAINT document_files_file_hash_sha256_length CHECK (octet_length(file_hash_sha256) = 32),
    CONSTRAINT document_files_storage_path_not_empty CHECK (trim(storage_path) <> ''),
    CONSTRAINT document_files_storage_bucket_not_empty CHECK (trim(storage_bucket) <> ''),

    -- Configuration and retention policy
    metadata                JSONB            NOT NULL DEFAULT '{}',
    keep_for_sec            INTEGER          DEFAULT NULL,
    auto_delete_at          TIMESTAMPTZ      DEFAULT NULL,

    CONSTRAINT document_files_metadata_size CHECK (length(metadata::TEXT) BETWEEN 2 AND 8192),
    CONSTRAINT document_files_retention_period CHECK (keep_for_sec IS NULL OR keep_for_sec BETWEEN 3600 AND 157680000),

    -- Lifecycle timestamps
    created_at              TIMESTAMPTZ      NOT NULL DEFAULT current_timestamp,
    updated_at              TIMESTAMPTZ      NOT NULL DEFAULT current_timestamp,
    deleted_at              TIMESTAMPTZ      DEFAULT NULL,

    CONSTRAINT document_files_updated_after_created CHECK (updated_at >= created_at),
    CONSTRAINT document_files_deleted_after_created CHECK (deleted_at IS NULL OR deleted_at >= created_at),
    CONSTRAINT document_files_deleted_after_updated CHECK (deleted_at IS NULL OR deleted_at >= updated_at),
    CONSTRAINT document_files_auto_delete_after_created CHECK (auto_delete_at IS NULL OR auto_delete_at > created_at)
);

-- Create auto-delete trigger function
CREATE OR REPLACE FUNCTION set_document_file_auto_delete()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.keep_for_sec IS NOT NULL THEN
        NEW.auto_delete_at := NEW.created_at + (NEW.keep_for_sec || ' seconds')::INTERVAL;
    ELSE
        NEW.auto_delete_at := NULL;
    END IF;
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
    WHERE processing_status = 'pending' AND deleted_at IS NULL;

CREATE INDEX document_files_hash_dedup_idx
    ON document_files (file_hash_sha256, file_size_bytes)
    WHERE deleted_at IS NULL;

CREATE INDEX document_files_cleanup_idx
    ON document_files (auto_delete_at)
    WHERE auto_delete_at IS NOT NULL AND deleted_at IS NULL;

CREATE INDEX document_files_tags_search_idx
    ON document_files USING gin (tags)
    WHERE array_length(tags, 1) > 0 AND deleted_at IS NULL;

CREATE INDEX document_files_indexed_idx
    ON document_files (is_indexed, content_segmentation)
    WHERE is_indexed = TRUE AND deleted_at IS NULL;

-- Add table and column comments
COMMENT ON TABLE document_files IS
    'Source files for document processing with pipeline management and security scanning.';

COMMENT ON COLUMN document_files.id IS 'Unique file identifier';
COMMENT ON COLUMN document_files.project_id IS 'Parent project reference (required)';
COMMENT ON COLUMN document_files.document_id IS 'Parent document reference (optional)';
COMMENT ON COLUMN document_files.account_id IS 'Uploading account reference';
COMMENT ON COLUMN document_files.display_name IS 'Display name (1-255 chars)';
COMMENT ON COLUMN document_files.original_filename IS 'Original upload filename (1-255 chars)';
COMMENT ON COLUMN document_files.file_extension IS 'File extension (1-20 alphanumeric)';
COMMENT ON COLUMN document_files.tags IS 'Classification tags (max 32)';
COMMENT ON COLUMN document_files.require_mode IS 'Processing mode required';
COMMENT ON COLUMN document_files.processing_priority IS 'Priority 1-10 (1=highest)';
COMMENT ON COLUMN document_files.processing_status IS 'Current processing status';
COMMENT ON COLUMN document_files.virus_scan_status IS 'Security scan result';
COMMENT ON COLUMN document_files.is_indexed IS 'Whether file content has been indexed for search';
COMMENT ON COLUMN document_files.content_segmentation IS 'Content segmentation strategy';
COMMENT ON COLUMN document_files.visual_support IS 'Whether to enable visual content processing';
COMMENT ON COLUMN document_files.file_size_bytes IS 'File size in bytes';
COMMENT ON COLUMN document_files.file_hash_sha256 IS 'SHA256 content hash';
COMMENT ON COLUMN document_files.storage_path IS 'Storage system path';
COMMENT ON COLUMN document_files.storage_bucket IS 'Storage bucket/container';
COMMENT ON COLUMN document_files.metadata IS 'Extended metadata (JSON, 2B-8KB)';
COMMENT ON COLUMN document_files.keep_for_sec IS 'Retention period (1h-5y)';
COMMENT ON COLUMN document_files.auto_delete_at IS 'Automatic deletion timestamp';
COMMENT ON COLUMN document_files.parent_id IS 'Parent file reference for hierarchical relationships';
COMMENT ON COLUMN document_files.created_at IS 'Upload timestamp';
COMMENT ON COLUMN document_files.updated_at IS 'Last modification timestamp';
COMMENT ON COLUMN document_files.deleted_at IS 'Soft deletion timestamp';

-- Create document comments table - User discussions and annotations
CREATE TABLE document_comments (
    -- Primary identifiers
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References
    file_id             UUID             NOT NULL REFERENCES document_files (id) ON DELETE CASCADE,
    account_id          UUID             NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,

    -- Thread references
    parent_comment_id   UUID             DEFAULT NULL REFERENCES document_comments (id) ON DELETE CASCADE,
    reply_to_account_id UUID             DEFAULT NULL REFERENCES accounts (id) ON DELETE SET NULL,

    -- Comment content
    content             TEXT             NOT NULL,

    CONSTRAINT document_comments_content_length CHECK (length(trim(content)) BETWEEN 1 AND 10000),

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
CREATE INDEX document_comments_file_idx
    ON document_comments (file_id, created_at DESC)
    WHERE deleted_at IS NULL;

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
    'User comments and discussions on files, supporting threaded conversations and @mentions.';

COMMENT ON COLUMN document_comments.id IS 'Unique comment identifier';
COMMENT ON COLUMN document_comments.file_id IS 'Parent file reference';
COMMENT ON COLUMN document_comments.account_id IS 'Comment author reference';
COMMENT ON COLUMN document_comments.parent_comment_id IS 'Parent comment for threaded replies (NULL for top-level)';
COMMENT ON COLUMN document_comments.reply_to_account_id IS 'Account being replied to (@mention)';
COMMENT ON COLUMN document_comments.content IS 'Comment text content (1-10000 chars)';
COMMENT ON COLUMN document_comments.metadata IS 'Extended metadata (JSON, 2B-4KB)';
COMMENT ON COLUMN document_comments.created_at IS 'Comment creation timestamp';
COMMENT ON COLUMN document_comments.updated_at IS 'Last edit timestamp';
COMMENT ON COLUMN document_comments.deleted_at IS 'Soft deletion timestamp';

-- Create document annotations table - Annotations for document content
CREATE TABLE document_annotations (
    -- Primary identifiers
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References
    document_file_id    UUID             NOT NULL REFERENCES document_files (id) ON DELETE CASCADE,
    account_id          UUID             NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,

    -- Annotation content
    content             TEXT             NOT NULL,
    annotation_type     TEXT             NOT NULL DEFAULT 'note',

    CONSTRAINT document_annotations_content_length CHECK (length(trim(content)) BETWEEN 1 AND 10000),
    CONSTRAINT document_annotations_type_format CHECK (annotation_type ~ '^[a-z_]+$'),

    -- Metadata
    metadata            JSONB            NOT NULL DEFAULT '{}',

    CONSTRAINT document_annotations_metadata_size CHECK (length(metadata::TEXT) BETWEEN 2 AND 4096),

    -- Lifecycle timestamps
    created_at          TIMESTAMPTZ      NOT NULL DEFAULT current_timestamp,
    updated_at          TIMESTAMPTZ      NOT NULL DEFAULT current_timestamp,
    deleted_at          TIMESTAMPTZ      DEFAULT NULL,

    CONSTRAINT document_annotations_updated_after_created CHECK (updated_at >= created_at),
    CONSTRAINT document_annotations_deleted_after_created CHECK (deleted_at IS NULL OR deleted_at >= created_at),
    CONSTRAINT document_annotations_deleted_after_updated CHECK (deleted_at IS NULL OR deleted_at >= updated_at)
);

-- Set up automatic updated_at trigger
SELECT setup_updated_at('document_annotations');

-- Create indexes for document annotations
CREATE INDEX document_annotations_file_idx
    ON document_annotations (document_file_id, created_at DESC)
    WHERE deleted_at IS NULL;

CREATE INDEX document_annotations_account_idx
    ON document_annotations (account_id, created_at DESC)
    WHERE deleted_at IS NULL;

CREATE INDEX document_annotations_type_idx
    ON document_annotations (annotation_type, document_file_id)
    WHERE deleted_at IS NULL;

-- Add table and column comments
COMMENT ON TABLE document_annotations IS
    'User annotations and highlights on document content.';

COMMENT ON COLUMN document_annotations.id IS 'Unique annotation identifier';
COMMENT ON COLUMN document_annotations.document_file_id IS 'Parent document file reference';
COMMENT ON COLUMN document_annotations.account_id IS 'Annotation author reference';
COMMENT ON COLUMN document_annotations.content IS 'Annotation text content (1-10000 chars)';
COMMENT ON COLUMN document_annotations.annotation_type IS 'Type of annotation (note, highlight, etc.)';
COMMENT ON COLUMN document_annotations.metadata IS 'Extended metadata including position/location (JSON, 2B-4KB)';
COMMENT ON COLUMN document_annotations.created_at IS 'Annotation creation timestamp';
COMMENT ON COLUMN document_annotations.updated_at IS 'Last edit timestamp';
COMMENT ON COLUMN document_annotations.deleted_at IS 'Soft deletion timestamp';

-- Create document processing summary view
CREATE VIEW document_processing_summary AS
SELECT
    d.id,
    d.display_name,
    d.status,
    d.project_id,
    COUNT(df.id) FILTER (WHERE df.deleted_at IS NULL) AS input_files_count,
    d.created_at,
    d.updated_at
FROM documents d
    LEFT JOIN document_files df ON d.id = df.document_id
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
WHERE df.processing_status IN ('pending', 'processing')
    AND df.deleted_at IS NULL
    AND d.deleted_at IS NULL
ORDER BY df.processing_priority DESC, df.created_at ASC;

COMMENT ON VIEW processing_queue IS
    'Files queued for processing, ordered by priority and age.';

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
