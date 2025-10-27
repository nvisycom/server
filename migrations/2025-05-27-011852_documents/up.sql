-- This migration creates tables for documents, files, processing pipeline, and security features

-- Create document status enum
CREATE TYPE DOCUMENT_STATUS AS ENUM (
    'draft', -- Document is being created/edited
    'processing', -- Document is being processed
    'ready', -- Document is ready for use
    'archived', -- Document is archived but accessible
    'locked', -- Document is locked for editing
    'error' -- Document processing failed
    );

COMMENT ON TYPE DOCUMENT_STATUS IS
    'Defines the current status of documents in the system.';

-- Create file processing status enum
CREATE TYPE PROCESSING_STATUS AS ENUM (
    'pending', -- File is queued for processing
    'processing', -- File is currently being processed
    'completed', -- Processing completed successfully
    'failed', -- Processing failed
    'canceled', -- Processing was canceled
    'skipped', -- Processing was skipped
    'retry' -- Processing is queued for retry
    );

COMMENT ON TYPE PROCESSING_STATUS IS
    'Defines the processing status for files in the pipeline.';

-- Create file type enum
CREATE TYPE FILE_TYPE AS ENUM (
    'document', -- Text documents (PDF, DOC, etc.)
    'image', -- Images (PNG, JPG, etc.)
    'video', -- Video files
    'audio', -- Audio files
    'archive', -- Compressed archives
    'data', -- Data files (CSV, JSON, etc.)
    'code' -- Source code files
    );

COMMENT ON TYPE FILE_TYPE IS
    'Categorizes files by their general type for processing and handling.';

-- Create require mode enum
CREATE TYPE REQUIRE_MODE AS ENUM (
    'text', -- Plain text content ready for analysis
    'ocr', -- Requires optical character recognition
    'transcribe', -- Requires audio/video transcription
    'mixed' -- May require multiple processing modes
    );

COMMENT ON TYPE REQUIRE_MODE IS
    'Defines the processing requirements for input files.';

-- Create virus scan status enum
CREATE TYPE VIRUS_SCAN_STATUS AS ENUM (
    'clean', -- No virus detected
    'infected', -- Virus detected
    'suspicious', -- Suspicious activity detected
    'unknown' -- Unable to determine status
    );

COMMENT ON TYPE VIRUS_SCAN_STATUS IS
    'Defines the possible outcomes of a virus scan.';

-- Create documents table (acts as folders/containers)
CREATE TABLE documents
(
    -- Primary identifiers
    id           UUID PRIMARY KEY         DEFAULT gen_random_uuid(),

    -- Foreign key references
    project_id   UUID            NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    account_id   UUID            NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,

    -- Document identity and organization
    display_name TEXT            NOT NULL DEFAULT 'Untitled',
    description  TEXT            NOT NULL DEFAULT '',
    tags         TEXT[]          NOT NULL DEFAULT '{}',

    CONSTRAINT documents_display_name_length_min CHECK (length(trim(display_name)) >= 1),
    CONSTRAINT documents_display_name_length_max CHECK (length(trim(display_name)) <= 255),
    CONSTRAINT documents_description_length_max CHECK (length(description) <= 2048),
    CONSTRAINT documents_tags_count_max CHECK (array_length(tags, 1) IS NULL OR array_length(tags, 1) <= 32),

    -- Document status and lifecycle
    status       DOCUMENT_STATUS NOT NULL DEFAULT 'draft',
    is_template  BOOLEAN         NOT NULL DEFAULT FALSE,

    -- Extended metadata and settings
    metadata     JSONB           NOT NULL DEFAULT '{}'::JSONB,
    settings     JSONB           NOT NULL DEFAULT '{}'::JSONB,

    CONSTRAINT documents_metadata_size_min CHECK (length(metadata::TEXT) >= 2),
    CONSTRAINT documents_metadata_size_max CHECK (length(metadata::TEXT) <= 16384),
    CONSTRAINT documents_settings_size_min CHECK (length(settings::TEXT) >= 2),
    CONSTRAINT documents_settings_size_max CHECK (length(settings::TEXT) <= 8192),

    -- Lifecycle timestamps
    created_at   TIMESTAMPTZ     NOT NULL DEFAULT current_timestamp,
    updated_at   TIMESTAMPTZ     NOT NULL DEFAULT current_timestamp,
    deleted_at   TIMESTAMPTZ              DEFAULT NULL,

    -- Chronological integrity constraints
    CONSTRAINT documents_updated_after_created CHECK (updated_at >= created_at),
    CONSTRAINT documents_deleted_after_created CHECK (deleted_at IS NULL OR deleted_at >= created_at),
    CONSTRAINT documents_deleted_after_updated CHECK (deleted_at IS NULL OR deleted_at >= updated_at)
);

-- Set up automatic updated_at trigger
SELECT setup_updated_at('documents');

-- Create comprehensive indexes for documents
CREATE INDEX documents_project_lookup_idx
    ON documents (project_id, status, created_at DESC)
    WHERE deleted_at IS NULL;

CREATE INDEX documents_account_lookup_idx
    ON documents (account_id, status, updated_at DESC)
    WHERE deleted_at IS NULL;

CREATE INDEX documents_status_lookup_idx
    ON documents (status, project_id, updated_at DESC)
    WHERE deleted_at IS NULL;

CREATE INDEX documents_tags_lookup_idx
    ON documents USING gin (tags)
    WHERE array_length(tags, 1) > 0 AND deleted_at IS NULL;

CREATE INDEX documents_metadata_lookup_idx
    ON documents USING gin (metadata)
    WHERE deleted_at IS NULL;

-- Add comprehensive table and column comments
COMMENT ON TABLE documents IS
    'Document containers (folders) with comprehensive metadata and settings.';

-- Primary identifiers
COMMENT ON COLUMN documents.id IS
    'Unique document identifier (UUID).';
COMMENT ON COLUMN documents.project_id IS
    'Reference to the project containing this document.';
COMMENT ON COLUMN documents.account_id IS
    'Reference to the account that created this document.';

-- Document identity and organization
COMMENT ON COLUMN documents.display_name IS
    'Human-readable document name (1-255 characters).';
COMMENT ON COLUMN documents.description IS
    'Detailed document description (up to 2048 characters).';
COMMENT ON COLUMN documents.tags IS
    'Array of tags for classification and search (max 32 tags).';

-- Document status and lifecycle
COMMENT ON COLUMN documents.status IS
    'Current document status in the lifecycle.';
COMMENT ON COLUMN documents.is_template IS
    'Mark document as template for creating new documents.';

-- Extended metadata and settings
COMMENT ON COLUMN documents.metadata IS
    'Extended document metadata (JSON, 2B-16KB).';
COMMENT ON COLUMN documents.settings IS
    'Document-specific settings and preferences (JSON, 2B-8KB).';

-- Lifecycle timestamps
COMMENT ON COLUMN documents.created_at IS
    'Timestamp when the document was created.';
COMMENT ON COLUMN documents.updated_at IS
    'Timestamp when the document was last modified.';
COMMENT ON COLUMN documents.deleted_at IS
    'Timestamp when the document was soft-deleted (NULL if active).';

-- Create document files table (source files for processing)
CREATE TABLE document_files
(
    -- Primary identifiers
    id                     UUID PRIMARY KEY           DEFAULT gen_random_uuid(),

    -- Foreign key references
    document_id            UUID              NOT NULL REFERENCES documents (id) ON DELETE CASCADE,
    account_id             UUID              NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,

    -- File identity and metadata
    display_name           TEXT              NOT NULL DEFAULT 'Untitled',
    original_filename      TEXT              NOT NULL DEFAULT 'Untitled',
    file_extension         TEXT              NOT NULL DEFAULT 'txt',
    mime_type              TEXT              NOT NULL DEFAULT 'text/plain',
    file_type              FILE_TYPE         NOT NULL DEFAULT 'document',

    CONSTRAINT document_files_display_name_length_min CHECK (length(trim(display_name)) >= 1),
    CONSTRAINT document_files_display_name_length_max CHECK (length(trim(display_name)) <= 255),
    CONSTRAINT document_files_original_filename_length_min CHECK (length(original_filename) >= 1),
    CONSTRAINT document_files_original_filename_length_max CHECK (length(original_filename) <= 255),
    CONSTRAINT document_files_file_extension_format CHECK (file_extension ~ '^[a-zA-Z0-9]{1,20}$'),
    CONSTRAINT document_files_mime_type_length_min CHECK (length(trim(mime_type)) >= 1),
    CONSTRAINT document_files_mime_type_length_max CHECK (length(trim(mime_type)) <= 255),

    -- File processing and metadata
    require_mode           REQUIRE_MODE      NOT NULL DEFAULT 'text',
    processing_priority    INTEGER           NOT NULL DEFAULT 5,
    metadata               JSONB             NOT NULL DEFAULT '{}'::JSONB,

    CONSTRAINT document_files_processing_priority_min CHECK (processing_priority >= 1),
    CONSTRAINT document_files_processing_priority_max CHECK (processing_priority <= 10),
    CONSTRAINT document_files_metadata_size_min CHECK (length(metadata::TEXT) >= 2),
    CONSTRAINT document_files_metadata_size_max CHECK (length(metadata::TEXT) <= 8192),

    -- File storage and integrity
    file_size_bytes        BIGINT            NOT NULL DEFAULT 0,
    storage_path           TEXT              NOT NULL,
    storage_bucket         TEXT              NOT NULL DEFAULT '',
    file_hash_sha256       BYTEA             NOT NULL,

    CONSTRAINT document_files_file_size_min CHECK (file_size_bytes >= 0),
    CONSTRAINT document_files_file_hash_sha256_length CHECK (octet_length(file_hash_sha256) = 32),
    CONSTRAINT document_files_storage_path_not_empty CHECK (trim(storage_path) <> ''),
    CONSTRAINT document_files_storage_bucket_not_empty CHECK (trim(storage_bucket) <> ''),

    -- Processing status and results
    processing_status      PROCESSING_STATUS NOT NULL DEFAULT 'pending',
    processing_attempts    INTEGER           NOT NULL DEFAULT 0,
    processing_error       TEXT                       DEFAULT NULL,
    processing_duration_ms INTEGER                    DEFAULT NULL,

    CONSTRAINT document_files_processing_attempts_min CHECK (processing_attempts >= 0),
    CONSTRAINT document_files_processing_attempts_max CHECK (processing_attempts <= 10),
    CONSTRAINT document_files_processing_error_length_max
        CHECK (processing_error IS NULL OR length(processing_error) <= 2000),
    CONSTRAINT document_files_processing_duration_min
        CHECK (processing_duration_ms IS NULL OR processing_duration_ms >= 0),

    -- Processing and content analysis metrics
    processing_score       DECIMAL(3, 2)     NOT NULL DEFAULT 0.00,
    completeness_score     DECIMAL(3, 2)     NOT NULL DEFAULT 0.00,
    confidence_score       DECIMAL(3, 2)     NOT NULL DEFAULT 0.00,

    CONSTRAINT document_files_processing_score_min CHECK (processing_score >= 0.00),
    CONSTRAINT document_files_processing_score_max CHECK (processing_score <= 1.00),
    CONSTRAINT document_files_completeness_score_min CHECK (completeness_score >= 0.00),
    CONSTRAINT document_files_completeness_score_max CHECK (completeness_score <= 1.00),
    CONSTRAINT document_files_confidence_score_min CHECK (confidence_score >= 0.00),
    CONSTRAINT document_files_confidence_score_max CHECK (confidence_score <= 1.00),

    -- Security and access
    is_sensitive           BOOLEAN           NOT NULL DEFAULT FALSE,
    is_encrypted           BOOLEAN           NOT NULL DEFAULT FALSE,
    virus_scan_status      VIRUS_SCAN_STATUS          DEFAULT NULL,

    -- Lifecycle timestamps
    created_at             TIMESTAMPTZ       NOT NULL DEFAULT current_timestamp,
    updated_at             TIMESTAMPTZ       NOT NULL DEFAULT current_timestamp,
    deleted_at             TIMESTAMPTZ                DEFAULT NULL,

    -- Chronological integrity constraints
    CONSTRAINT document_files_updated_after_created CHECK (updated_at >= created_at),
    CONSTRAINT document_files_deleted_after_created CHECK (deleted_at IS NULL OR deleted_at >= created_at),
    CONSTRAINT document_files_deleted_after_updated CHECK (deleted_at IS NULL OR deleted_at >= updated_at),
    CONSTRAINT document_files_auto_delete_after_created CHECK (auto_delete_at IS NULL OR auto_delete_at > created_at),

    -- Retention and lifecycle
    keep_for_sec           INTEGER           NOT NULL DEFAULT 31536000,
    auto_delete_at         TIMESTAMPTZ                DEFAULT NULL,

    CONSTRAINT document_files_retention_period_min CHECK (keep_for_sec >= 3600),
    CONSTRAINT document_files_retention_period_max CHECK (keep_for_sec <= 157680000)
);

-- Add trigger to set auto_delete_at on insert/update
CREATE OR REPLACE FUNCTION set_document_file_auto_delete() RETURNS TRIGGER AS
$$
BEGIN
    new.auto_delete_at := new.created_at + (new.keep_for_sec || ' seconds')::INTERVAL;
    RETURN new;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_document_files_auto_delete
    BEFORE INSERT OR UPDATE OF keep_for_sec
    ON document_files
    FOR EACH ROW
EXECUTE FUNCTION set_document_file_auto_delete();

-- Set up automatic updated_at trigger
SELECT setup_updated_at('document_files');

-- Create indexes for input files
CREATE INDEX document_files_document_idx
    ON document_files (document_id, processing_status, created_at DESC)
    WHERE deleted_at IS NULL;

CREATE INDEX document_files_processing_queue_idx
    ON document_files (processing_status, processing_priority DESC, created_at ASC)
    WHERE processing_status IN ('pending', 'retry') AND deleted_at IS NULL;

CREATE INDEX document_files_hash_lookup_idx
    ON document_files (file_hash_sha256, file_size_bytes)
    WHERE deleted_at IS NULL;

CREATE INDEX document_files_type_lookup_idx
    ON document_files (file_type, require_mode, processing_status)
    WHERE deleted_at IS NULL;

CREATE INDEX document_files_cleanup_idx
    ON document_files (auto_delete_at)
    WHERE auto_delete_at IS NOT NULL AND deleted_at IS NULL;

-- Add comprehensive table and column comments
COMMENT ON TABLE document_files IS
    'Source files for document processing with comprehensive pipeline management, security scanning, and metadata tracking.';

-- Primary identifiers
COMMENT ON COLUMN document_files.id IS
    'Unique file identifier (UUID).';
COMMENT ON COLUMN document_files.document_id IS
    'Reference to the parent document container.';
COMMENT ON COLUMN document_files.account_id IS
    'Reference to the account that uploaded this file.';

-- File identity and metadata
COMMENT ON COLUMN document_files.display_name IS
    'Human-readable file name for display purposes (1-255 characters).';
COMMENT ON COLUMN document_files.original_filename IS
    'Original filename as uploaded by the user (1-255 characters).';
COMMENT ON COLUMN document_files.file_extension IS
    'File extension without the dot (e.g., "pdf", "docx").';
COMMENT ON COLUMN document_files.mime_type IS
    'MIME type of the uploaded file (e.g., "application/pdf").';
COMMENT ON COLUMN document_files.file_type IS
    'High-level categorization of the file type.';

-- File processing requirements
COMMENT ON COLUMN document_files.require_mode IS
    'Processing mode required for this file type.';
COMMENT ON COLUMN document_files.processing_priority IS
    'Processing priority from 1 (highest) to 10 (lowest).';
COMMENT ON COLUMN document_files.metadata IS
    'Extended file metadata and processing results (JSON, 2B-8KB).';

-- File storage and integrity
COMMENT ON COLUMN document_files.file_size_bytes IS
    'File size in bytes for storage and quota tracking.';
COMMENT ON COLUMN document_files.storage_path IS
    'Path to the file in the storage system.';
COMMENT ON COLUMN document_files.storage_bucket IS
    'Storage bucket or container name.';
COMMENT ON COLUMN document_files.file_hash_sha256 IS
    'SHA256 hash of the file content for integrity verification.';

-- Processing status and results
COMMENT ON COLUMN document_files.processing_status IS
    'Current status of file processing.';
COMMENT ON COLUMN document_files.processing_attempts IS
    'Number of processing attempts made (0-10).';
COMMENT ON COLUMN document_files.processing_error IS
    'Last processing error message if processing failed.';
COMMENT ON COLUMN document_files.processing_duration_ms IS
    'Duration of last processing attempt in milliseconds.';

-- Processing and content analysis metrics
COMMENT ON COLUMN document_files.processing_score IS
    'Overall processing quality score (0.00-1.00).';
COMMENT ON COLUMN document_files.completeness_score IS
    'Content extraction completeness score (0.00-1.00).';
COMMENT ON COLUMN document_files.confidence_score IS
    'Processing confidence score (0.00-1.00).';

-- Security and access
COMMENT ON COLUMN document_files.is_sensitive IS
    'Flag indicating if the file contains sensitive information.';
COMMENT ON COLUMN document_files.is_encrypted IS
    'Flag indicating if the file is encrypted at rest.';
COMMENT ON COLUMN document_files.virus_scan_status IS
    'Result of the last virus scan.';

-- Lifecycle timestamps
COMMENT ON COLUMN document_files.created_at IS
    'Timestamp when the file was uploaded.';
COMMENT ON COLUMN document_files.updated_at IS
    'Timestamp when the file record was last modified.';
COMMENT ON COLUMN document_files.deleted_at IS
    'Timestamp when the file was soft-deleted (NULL if active).';

-- Retention and lifecycle
COMMENT ON COLUMN document_files.keep_for_sec IS
    'Retention period in seconds (1 hour to 5 years).';
COMMENT ON COLUMN document_files.auto_delete_at IS
    'Automatic deletion timestamp based on retention policy.';

-- Create document versions table (processed versions of documents)
CREATE TABLE document_versions
(
    -- Primary identifiers
    id                     UUID PRIMARY KEY       DEFAULT gen_random_uuid(),

    -- Foreign key references
    document_id            UUID          NOT NULL REFERENCES documents (id) ON DELETE CASCADE,
    account_id             UUID          NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    version_number         INTEGER       NOT NULL,

    CONSTRAINT document_versions_version_number_min CHECK (version_number >= 1),

    -- File identity and metadata
    display_name           TEXT          NOT NULL DEFAULT 'Untitled',
    file_extension         TEXT          NOT NULL DEFAULT 'txt',
    mime_type              TEXT          NOT NULL DEFAULT 'text/plain',
    file_type              FILE_TYPE     NOT NULL DEFAULT 'document',

    CONSTRAINT document_versions_display_name_length_min CHECK (length(trim(display_name)) >= 1),
    CONSTRAINT document_versions_display_name_length_max CHECK (length(trim(display_name)) <= 255),
    CONSTRAINT document_versions_file_extension_format CHECK (file_extension ~ '^[a-zA-Z0-9]{1,20}$'),
    CONSTRAINT document_versions_mime_type_not_empty CHECK (trim(mime_type) <> ''),

    -- Processing metrics and costs
    processing_credits     INTEGER       NOT NULL DEFAULT 0,
    processing_duration_ms INTEGER       NOT NULL DEFAULT 0,
    processing_cost_usd    DECIMAL(8, 4)          DEFAULT NULL,
    api_calls_made         INTEGER       NOT NULL DEFAULT 0,

    CONSTRAINT document_versions_processing_credits_min CHECK (processing_credits >= 0),
    CONSTRAINT document_versions_processing_duration_min CHECK (processing_duration_ms >= 0),
    CONSTRAINT document_versions_processing_cost_min CHECK (processing_cost_usd IS NULL OR processing_cost_usd >= 0),
    CONSTRAINT document_versions_api_calls_min CHECK (api_calls_made >= 0),

    -- Quality and analysis metrics
    accuracy_score         DECIMAL(3, 2) NOT NULL DEFAULT 0.00,
    completeness_score     DECIMAL(3, 2) NOT NULL DEFAULT 0.00,
    confidence_score       DECIMAL(3, 2) NOT NULL DEFAULT 0.00,

    CONSTRAINT document_versions_accuracy_score_min CHECK (accuracy_score >= 0.00),
    CONSTRAINT document_versions_accuracy_score_max CHECK (accuracy_score <= 1.00),
    CONSTRAINT document_versions_completeness_score_min CHECK (completeness_score >= 0.00),
    CONSTRAINT document_versions_completeness_score_max CHECK (completeness_score <= 1.00),
    CONSTRAINT document_versions_confidence_score_min CHECK (confidence_score >= 0.00),
    CONSTRAINT document_versions_confidence_score_max CHECK (confidence_score <= 1.00),

    -- File storage and integrity
    file_size_bytes        BIGINT        NOT NULL DEFAULT 0,
    storage_path           TEXT          NOT NULL,
    storage_bucket         TEXT          NOT NULL DEFAULT '',
    file_hash_sha256       BYTEA         NOT NULL,

    CONSTRAINT document_versions_file_size_min CHECK (file_size_bytes >= 0),
    CONSTRAINT document_versions_storage_path_not_empty CHECK (trim(storage_path) <> ''),
    CONSTRAINT document_versions_storage_bucket_not_empty CHECK (trim(storage_bucket) <> ''),
    CONSTRAINT document_versions_file_hash_sha256_length CHECK (octet_length(file_hash_sha256) = 32),

    -- Security and access
    is_encrypted           BOOLEAN       NOT NULL DEFAULT FALSE,
    encryption_key_id      TEXT                   DEFAULT NULL,

    -- Processing results and metadata
    processing_results     JSONB         NOT NULL DEFAULT '{}'::JSONB,
    metadata               JSONB         NOT NULL DEFAULT '{}'::JSONB,

    CONSTRAINT document_versions_processing_results_size_min CHECK (length(processing_results::TEXT) >= 2),
    CONSTRAINT document_versions_processing_results_size_max CHECK (length(processing_results::TEXT) <= 65536),
    CONSTRAINT document_versions_metadata_size_min CHECK (length(metadata::TEXT) >= 2),
    CONSTRAINT document_versions_metadata_size_max CHECK (length(metadata::TEXT) <= 16384),

    -- Lifecycle timestamps
    created_at             TIMESTAMPTZ   NOT NULL DEFAULT current_timestamp,
    updated_at             TIMESTAMPTZ   NOT NULL DEFAULT current_timestamp,
    deleted_at             TIMESTAMPTZ            DEFAULT NULL,

    -- Chronological integrity constraints
    CONSTRAINT document_versions_updated_after_created CHECK (updated_at >= created_at),
    CONSTRAINT document_versions_deleted_after_created CHECK (deleted_at IS NULL OR deleted_at >= created_at),
    CONSTRAINT document_versions_deleted_after_updated CHECK (deleted_at IS NULL OR deleted_at >= updated_at),

    -- Retention and lifecycle
    keep_for_sec           INTEGER       NOT NULL DEFAULT 31536000,
    auto_delete_at         TIMESTAMPTZ            DEFAULT NULL,

    CONSTRAINT document_versions_retention_period_min CHECK (keep_for_sec >= 3600),
    CONSTRAINT document_versions_retention_period_max CHECK (keep_for_sec <= 157680000),
    CONSTRAINT document_versions_auto_delete_after_created CHECK (auto_delete_at IS NULL OR auto_delete_at > created_at),

    -- Unique constraint per document version
    CONSTRAINT document_versions_unique_version UNIQUE (document_id, version_number)
);

-- Add trigger to set auto_delete_at on insert/update for versions
CREATE OR REPLACE FUNCTION set_document_version_auto_delete() RETURNS TRIGGER AS
$$
BEGIN
    new.auto_delete_at := new.created_at + (new.keep_for_sec || ' seconds')::INTERVAL;
    RETURN new;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_document_versions_auto_delete
    BEFORE INSERT OR UPDATE OF keep_for_sec
    ON document_versions
    FOR EACH ROW
EXECUTE FUNCTION set_document_version_auto_delete();

-- Set up automatic updated_at trigger
SELECT setup_updated_at('document_versions');

-- Create indexes for document versions
CREATE INDEX document_versions_document_idx
    ON document_versions (document_id, version_number DESC)
    WHERE deleted_at IS NULL;

CREATE INDEX document_versions_account_idx
    ON document_versions (account_id, created_at DESC)
    WHERE deleted_at IS NULL;

CREATE INDEX document_versions_quality_idx
    ON document_versions (accuracy_score DESC, completeness_score DESC)
    WHERE deleted_at IS NULL;

CREATE INDEX document_versions_cleanup_idx
    ON document_versions (auto_delete_at)
    WHERE auto_delete_at IS NOT NULL AND deleted_at IS NULL;

CREATE INDEX document_versions_hash_dedup_idx
    ON document_versions (file_hash_sha256, file_size_bytes)
    WHERE deleted_at IS NULL;

CREATE INDEX document_versions_cost_tracking_idx
    ON document_versions (processing_cost_usd, processing_credits, created_at DESC)
    WHERE processing_cost_usd IS NOT NULL;

-- Add comprehensive table and column comments
COMMENT ON TABLE document_versions IS
    'Processed document versions with comprehensive metrics, quality scores, and content analysis results.';

-- Primary identifiers
COMMENT ON COLUMN document_versions.id IS
    'Unique version identifier (UUID).';
COMMENT ON COLUMN document_versions.document_id IS
    'Reference to the parent document container.';
COMMENT ON COLUMN document_versions.account_id IS
    'Reference to the account that created this version (processing initiator).';
COMMENT ON COLUMN document_versions.version_number IS
    'Sequential version number starting from 1.';

-- File identity and metadata
COMMENT ON COLUMN document_versions.display_name IS
    'Human-readable version name for display purposes (1-255 characters).';
COMMENT ON COLUMN document_versions.file_extension IS
    'File extension of the processed version (e.g., "pdf", "txt").';
COMMENT ON COLUMN document_versions.mime_type IS
    'MIME type of the processed version output.';
COMMENT ON COLUMN document_versions.file_type IS
    'High-level categorization of the processed file type.';

-- Processing metrics and costs
COMMENT ON COLUMN document_versions.processing_credits IS
    'Number of processing credits consumed for this version.';
COMMENT ON COLUMN document_versions.processing_duration_ms IS
    'Total processing time in milliseconds.';
COMMENT ON COLUMN document_versions.processing_cost_usd IS
    'Processing cost in USD for external API calls.';
COMMENT ON COLUMN document_versions.api_calls_made IS
    'Number of external API calls made during processing.';

-- Quality and analysis metrics
COMMENT ON COLUMN document_versions.accuracy_score IS
    'Processing accuracy score (0.00-1.00).';
COMMENT ON COLUMN document_versions.completeness_score IS
    'Content extraction completeness score (0.00-1.00).';
COMMENT ON COLUMN document_versions.confidence_score IS
    'Processing confidence score (0.00-1.00).';

-- File storage and integrity
COMMENT ON COLUMN document_versions.file_size_bytes IS
    'File size of the processed version in bytes.';
COMMENT ON COLUMN document_versions.storage_path IS
    'Path to the processed version in the storage system.';
COMMENT ON COLUMN document_versions.storage_bucket IS
    'Storage bucket or container name for the processed version.';
COMMENT ON COLUMN document_versions.file_hash_sha256 IS
    'SHA256 hash of the processed version content.';

-- Security and access
COMMENT ON COLUMN document_versions.is_encrypted IS
    'Flag indicating if the processed version is encrypted at rest.';
COMMENT ON COLUMN document_versions.encryption_key_id IS
    'Reference to the encryption key used (if encrypted).';

-- Processing results and metadata
COMMENT ON COLUMN document_versions.processing_results IS
    'Detailed processing results and analysis data (JSON, 2B-64KB).';
COMMENT ON COLUMN document_versions.metadata IS
    'Extended version metadata and analysis results (JSON, 2B-16KB).';

-- Lifecycle timestamps
COMMENT ON COLUMN document_versions.created_at IS
    'Timestamp when the version was created/processed.';
COMMENT ON COLUMN document_versions.updated_at IS
    'Timestamp when the version record was last modified.';
COMMENT ON COLUMN document_versions.deleted_at IS
    'Timestamp when the version was soft-deleted (NULL if active).';

-- Retention and lifecycle
COMMENT ON COLUMN document_versions.keep_for_sec IS
    'Retention period in seconds (1 hour to 5 years).';
COMMENT ON COLUMN document_versions.auto_delete_at IS
    'Automatic deletion timestamp based on retention policy.';

-- Create useful views for document management
CREATE VIEW document_processing_summary AS
SELECT d.id,
       d.display_name,
       d.status,
       count(df.id)                AS input_files_count,
       count(dv.id)                AS output_files_count,
       sum(dv.processing_credits)  AS total_credits_used,
       sum(dv.processing_cost_usd) AS total_cost_usd,
       max(dv.accuracy_score)      AS best_accuracy_score,
       max(dv.created_at)          AS latest_version_at
FROM documents d
         LEFT JOIN document_files df ON d.id = df.document_id AND df.deleted_at IS NULL
         LEFT JOIN document_versions dv ON d.id = dv.document_id AND dv.deleted_at IS NULL
WHERE d.deleted_at IS NULL
GROUP BY d.id, d.display_name, d.status;

COMMENT ON VIEW document_processing_summary IS
    'Summary view of document processing status, metrics, and costs.';

CREATE VIEW pending_file_processing AS
SELECT df.id,
       df.document_id,
       d.display_name                                          AS document_name,
       d.project_id,
       df.display_name                                         AS file_name,
       df.file_type,
       df.require_mode,
       df.processing_priority,
       df.processing_status,
       df.processing_attempts,
       df.file_size_bytes,
       df.created_at,
       extract(EPOCH FROM (current_timestamp - df.created_at)) AS queue_time_seconds
FROM document_files df
         JOIN documents d ON df.document_id = d.id
WHERE df.processing_status IN ('pending', 'retry', 'processing')
  AND df.deleted_at IS NULL
  AND d.deleted_at IS NULL
ORDER BY df.processing_priority DESC, df.created_at ASC;

COMMENT ON VIEW pending_file_processing IS
    'Queue of files pending processing, ordered by priority and age.';

-- Function to create document version
CREATE OR REPLACE FUNCTION create_document_version(
    _document_id UUID,
    _account_id UUID
) RETURNS UUID AS
$$
DECLARE
    _version_id     UUID;
    _version_number INTEGER;
BEGIN
    -- Get next version number
    SELECT coalesce(max(version_number), 0) + 1
    INTO _version_number
    FROM document_versions
    WHERE document_id = _document_id;

    -- Create version record
    INSERT INTO document_versions (document_id, account_id, version_number)
    VALUES (_document_id, _account_id, _version_number)
    RETURNING id INTO _version_id;

    RETURN _version_id;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION create_document_version(UUID, UUID) IS
    'Creates a new version of a document.';

-- Function to cleanup old document files
CREATE OR REPLACE FUNCTION cleanup_expired_document_files()
    RETURNS TABLE
            (
                FILES_CLEANED    INTEGER,
                VERSIONS_CLEANED INTEGER,
                STORAGE_FREED_MB DECIMAL
            )
AS
$$
DECLARE
    file_count    INTEGER;
    version_count INTEGER;
    storage_freed DECIMAL;
BEGIN
    -- Clean up expired files
    WITH deleted_files AS (
        UPDATE document_files
            SET deleted_at = current_timestamp,
                updated_at = current_timestamp
            WHERE (auto_delete_at < current_timestamp OR
                   (extract(EPOCH FROM (current_timestamp - created_at)) > keep_for_sec))
                AND deleted_at IS NULL
            RETURNING file_size_bytes)
    SELECT count(*), round(coalesce(sum(file_size_bytes), 0) / 1048576.0, 2)
    INTO file_count, storage_freed
    FROM deleted_files;

    -- Clean up expired versions
    WITH deleted_versions AS (
        UPDATE document_versions
            SET deleted_at = current_timestamp,
                updated_at = current_timestamp
            WHERE (auto_delete_at < current_timestamp OR
                   (extract(EPOCH FROM (current_timestamp - created_at)) > keep_for_sec))
                AND deleted_at IS NULL
            RETURNING file_size_bytes)
    SELECT count(*), storage_freed + round(coalesce(sum(file_size_bytes), 0) / 1048576.0, 2)
    INTO version_count, storage_freed
    FROM deleted_versions;

    -- Return cleanup results
    RETURN QUERY SELECT file_count, version_count, storage_freed;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION cleanup_expired_document_files() IS
    'Cleans up expired document files based on retention policies. Returns cleanup statistics.';

-- Function to detect duplicate files
CREATE OR REPLACE FUNCTION find_duplicate_files(_document_id UUID DEFAULT NULL)
    RETURNS TABLE
            (
                FILE_HASH       TEXT,
                FILE_SIZE       BIGINT,
                DUPLICATE_COUNT BIGINT,
                FILE_IDS        UUID[]
            )
AS
$$
BEGIN
    RETURN QUERY
        SELECT encode(df.file_hash_sha256, 'hex'),
               df.file_size_bytes,
               count(*)         AS duplicate_count,
               array_agg(df.id) AS file_ids
        FROM document_files df
        WHERE (_document_id IS NULL OR df.document_id = _document_id)
          AND df.deleted_at IS NULL
        GROUP BY df.file_hash_sha256, df.file_size_bytes
        HAVING count(*) > 1
        ORDER BY count(*) DESC;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION find_duplicate_files(UUID) IS
    'Finds duplicate files by hash and size. Optionally scoped to a specific document.';
