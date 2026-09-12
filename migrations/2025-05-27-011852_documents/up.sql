-- Storage split: blobs (raw bytes) and workspace_documents (human-facing files).
--
-- A blob is content-addressed, shared, and ref-counted: identical bytes are
-- stored once and referenced by every document, audit, or intermediate that
-- needs them. A blob owns its data-retention window; the reaper reclaims a blob
-- only once nothing references it (ref_count = 0) and its window has elapsed.
--
-- A document is a human-facing file — an original the user uploaded or a redacted
-- output — pointing at its blob. Machine byproducts (detection audits, review
-- audits, enrichment intermediates) are not documents; they reference blobs
-- directly from their own tables.

-- Raw stored bytes, deduplicated by content within a workspace and ref-counted.
CREATE TABLE workspace_blobs (
    -- Primary identifier
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Owning workspace (scopes the encryption key and dedup lookup).
    workspace_id            UUID             NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,

    -- Content identity and storage location.
    content_hash            BYTEA            NOT NULL,
    file_size_bytes         BIGINT           NOT NULL,
    storage_path            TEXT             NOT NULL,
    storage_bucket          TEXT             NOT NULL,
    CONSTRAINT blobs_content_hash_length CHECK (octet_length(content_hash) = 32),
    CONSTRAINT blobs_file_size_min CHECK (file_size_bytes >= 0),
    CONSTRAINT blobs_storage_path_not_empty CHECK (trim(storage_path) <> ''),
    CONSTRAINT blobs_storage_bucket_not_empty CHECK (trim(storage_bucket) <> ''),

    -- How many live rows reference this blob. The reaper reclaims a blob only at
    -- zero. Never negative.
    ref_count               INTEGER          NOT NULL DEFAULT 0,
    CONSTRAINT blobs_ref_count_min CHECK (ref_count >= 0),

    -- Lifecycle: created, retention window, and object reclamation. Reclamation is
    -- two-phase: purged_at claims the blob (committed before the object delete, so
    -- it stops deduplicating onto missing content), reclaimed_at confirms the
    -- object was removed. A claimed-but-not-reclaimed blob is retried by reconcile.
    created_at              TIMESTAMPTZ      NOT NULL DEFAULT current_timestamp,
    expires_at              TIMESTAMPTZ      DEFAULT NULL,
    purged_at               TIMESTAMPTZ      DEFAULT NULL,
    reclaimed_at            TIMESTAMPTZ      DEFAULT NULL,
    CONSTRAINT blobs_expires_after_created CHECK (expires_at IS NULL OR expires_at >= created_at),
    CONSTRAINT blobs_purged_after_created CHECK (purged_at IS NULL OR purged_at >= created_at),
    CONSTRAINT blobs_reclaimed_after_purged CHECK (reclaimed_at IS NULL OR purged_at IS NOT NULL),

    -- Composite unique target so a referrer's (workspace_id, id) foreign key can
    -- enforce that a referenced blob belongs to the referrer's workspace.
    CONSTRAINT workspace_blobs_workspace_id_id_key UNIQUE (workspace_id, id)
);

-- Deduplication lookup: an incoming blob reuses a live match on this key. Unique
-- over unclaimed blobs so concurrent identical uploads converge on one row via
-- ON CONFLICT rather than inserting duplicates. storage_bucket is part of the
-- identity so byte-identical content in different buckets stays distinct (each
-- bucket's objects are read with a different key type).
CREATE UNIQUE INDEX blobs_dedup_idx
    ON workspace_blobs (workspace_id, content_hash, file_size_bytes, storage_bucket)
    WHERE purged_at IS NULL;

-- Data-retention sweep: the reclaimable set — unreferenced, expired, not yet
-- claimed.
CREATE INDEX blobs_reclaimable_idx
    ON workspace_blobs (expires_at)
    WHERE ref_count = 0 AND expires_at IS NOT NULL AND purged_at IS NULL;

-- Reconcile sweep: blobs claimed for purge whose object delete has not been
-- confirmed, retried until reclaimed_at is stamped.
CREATE INDEX blobs_pending_reclaim_idx
    ON workspace_blobs (purged_at)
    WHERE purged_at IS NOT NULL AND reclaimed_at IS NULL;

COMMENT ON TABLE workspace_blobs IS 'Content-addressed, ref-counted raw bytes shared across documents, audits, and intermediates; owns the data-retention window.';
COMMENT ON COLUMN workspace_blobs.id IS 'Unique blob identifier';
COMMENT ON COLUMN workspace_blobs.workspace_id IS 'Owning workspace (scopes encryption and dedup)';
COMMENT ON COLUMN workspace_blobs.content_hash IS 'SHA256 content hash';
COMMENT ON COLUMN workspace_blobs.file_size_bytes IS 'Plaintext size in bytes';
COMMENT ON COLUMN workspace_blobs.storage_path IS 'Object key in the store';
COMMENT ON COLUMN workspace_blobs.storage_bucket IS 'Storage bucket/container';
COMMENT ON COLUMN workspace_blobs.ref_count IS 'Number of live rows referencing this blob; reclaimed at zero';
COMMENT ON COLUMN workspace_blobs.created_at IS 'Creation timestamp';
COMMENT ON COLUMN workspace_blobs.expires_at IS 'Data-retention expiry (NULL = keep indefinitely)';
COMMENT ON COLUMN workspace_blobs.purged_at IS 'When the blob was claimed for purge (excluded from dedup); NULL means still live';
COMMENT ON COLUMN workspace_blobs.reclaimed_at IS 'When the backing object was confirmed removed; NULL means the delete is still pending';

-- Kind of a document: the source a user uploaded, or a redacted output.
CREATE TYPE DOCUMENT_KIND AS ENUM (
    'original',  -- Source document (uploaded or imported)
    'redacted'   -- Redacted output produced by a redaction
);

COMMENT ON TYPE DOCUMENT_KIND IS 'The kind of a human-facing document: an original source or a redacted output.';

-- Human-facing files: originals and redacted outputs, each pointing at a blob.
CREATE TABLE workspace_documents (
    -- Primary identifier
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References
    workspace_id            UUID             NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    account_id              UUID             NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    blob_id                 UUID             NOT NULL REFERENCES workspace_blobs (id),

    -- Composite key target for workspace-scoped foreign keys.
    CONSTRAINT workspace_documents_workspace_id_id_key UNIQUE (workspace_id, id),

    -- Kind and metadata
    kind                    DOCUMENT_KIND    NOT NULL DEFAULT 'original',
    display_name            TEXT             NOT NULL DEFAULT 'Untitled',
    original_filename       TEXT             NOT NULL DEFAULT 'Untitled',
    file_extension          TEXT             NOT NULL DEFAULT 'txt',
    CONSTRAINT workspace_documents_display_name_length CHECK (length(trim(display_name)) BETWEEN 1 AND 255),
    CONSTRAINT workspace_documents_original_filename_length CHECK (length(original_filename) BETWEEN 1 AND 255),
    CONSTRAINT workspace_documents_file_extension_format CHECK (file_extension ~ '^[a-zA-Z0-9]{1,20}$'),

    -- Configuration
    metadata                JSONB            NOT NULL DEFAULT '{}',
    CONSTRAINT workspace_documents_metadata_size CHECK (length(metadata::TEXT) BETWEEN 2 AND 65536),

    -- Lifecycle timestamps
    created_at              TIMESTAMPTZ      NOT NULL DEFAULT current_timestamp,
    updated_at              TIMESTAMPTZ      NOT NULL DEFAULT current_timestamp,
    deleted_at              TIMESTAMPTZ      DEFAULT NULL,
    CONSTRAINT workspace_documents_updated_after_created CHECK (updated_at >= created_at),
    CONSTRAINT workspace_documents_deleted_after_created CHECK (deleted_at IS NULL OR deleted_at >= created_at)
);

-- Keep updated_at current on every row modification.
SELECT setup_updated_at('workspace_documents');

-- Most recent live documents per workspace (the document list).
CREATE INDEX workspace_documents_workspace_idx
    ON workspace_documents (workspace_id, created_at DESC)
    WHERE deleted_at IS NULL;

-- Most recent live documents per account.
CREATE INDEX workspace_documents_account_idx
    ON workspace_documents (account_id, created_at DESC)
    WHERE deleted_at IS NULL;

-- Back the blob foreign key (ref-count maintenance and reclamation walks it).
CREATE INDEX workspace_documents_blob_idx
    ON workspace_documents (blob_id)
    WHERE deleted_at IS NULL;

-- Fuzzy display-name search over live documents.
CREATE INDEX workspace_documents_display_name_trgm_idx
    ON workspace_documents USING gin (display_name gin_trgm_ops)
    WHERE deleted_at IS NULL;

COMMENT ON TABLE workspace_documents IS 'Human-facing files (original sources and redacted outputs) pointing at their blob.';
COMMENT ON COLUMN workspace_documents.id IS 'Unique document identifier';
COMMENT ON COLUMN workspace_documents.workspace_id IS 'Parent workspace reference';
COMMENT ON COLUMN workspace_documents.account_id IS 'Uploading/creating account reference';
COMMENT ON COLUMN workspace_documents.blob_id IS 'Blob holding the document bytes';
COMMENT ON COLUMN workspace_documents.kind IS 'Document kind (original or redacted)';
COMMENT ON COLUMN workspace_documents.display_name IS 'Display name (1-255 chars)';
COMMENT ON COLUMN workspace_documents.original_filename IS 'Original upload filename (1-255 chars)';
COMMENT ON COLUMN workspace_documents.file_extension IS 'File extension (1-20 alphanumeric); Content-Type is derived from it';
COMMENT ON COLUMN workspace_documents.metadata IS 'Extended metadata (JSON)';
COMMENT ON COLUMN workspace_documents.created_at IS 'Creation timestamp';
COMMENT ON COLUMN workspace_documents.updated_at IS 'Last modification timestamp';
COMMENT ON COLUMN workspace_documents.deleted_at IS 'Soft-deletion timestamp; NULL means live';

-- Document lifecycle events feed the activity log and webhooks.
ALTER TYPE ACTIVITY_TYPE ADD VALUE IF NOT EXISTS 'document.created';
ALTER TYPE ACTIVITY_TYPE ADD VALUE IF NOT EXISTS 'document.updated';
ALTER TYPE ACTIVITY_TYPE ADD VALUE IF NOT EXISTS 'document.deleted';

ALTER TYPE WEBHOOK_EVENT ADD VALUE IF NOT EXISTS 'document.created';
ALTER TYPE WEBHOOK_EVENT ADD VALUE IF NOT EXISTS 'document.updated';
ALTER TYPE WEBHOOK_EVENT ADD VALUE IF NOT EXISTS 'document.deleted';
