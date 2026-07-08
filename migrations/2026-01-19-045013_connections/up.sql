-- Connections: encrypted provider connections scoped to workspaces.

-- Sync status enum for connections
CREATE TYPE SYNC_STATUS AS ENUM (
    'pending',      -- Sync is pending
    'running',      -- Sync is in progress
    'cancelled'     -- Sync was cancelled
);

COMMENT ON TYPE SYNC_STATUS IS
    'Status for connection sync operations.';

-- Workspace connections table (encrypted provider credentials + context)
CREATE TABLE workspace_connections (
    -- Primary identifier
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References
    workspace_id    UUID            NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    account_id      UUID            NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,

    -- Core attributes
    name            TEXT            NOT NULL,
    provider        TEXT            NOT NULL,

    CONSTRAINT workspace_connections_name_length CHECK (length(trim(name)) BETWEEN 1 AND 255),
    CONSTRAINT workspace_connections_provider_length CHECK (length(trim(provider)) BETWEEN 1 AND 64),

    -- Encrypted connection data (XChaCha20-Poly1305 encrypted JSON)
    -- Contains: {"type": "postgres", "credentials": {...}, "context": {...}}
    -- The context includes resumption state (last cursor, offset, etc.)
    encrypted_data  BYTEA           NOT NULL,

    CONSTRAINT workspace_connections_data_size CHECK (length(encrypted_data) BETWEEN 1 AND 65536),

    -- Sync status
    is_active       BOOLEAN         NOT NULL DEFAULT TRUE,
    last_sync_at    TIMESTAMPTZ     DEFAULT NULL,
    sync_status     SYNC_STATUS     DEFAULT NULL,

    -- Metadata (non-encrypted, for filtering/display)
    metadata        JSONB           NOT NULL DEFAULT '{}',

    CONSTRAINT workspace_connections_metadata_size CHECK (length(metadata::TEXT) BETWEEN 2 AND 65536),

    -- Lifecycle timestamps
    created_at      TIMESTAMPTZ     NOT NULL DEFAULT current_timestamp,
    updated_at      TIMESTAMPTZ     NOT NULL DEFAULT current_timestamp,
    deleted_at      TIMESTAMPTZ     DEFAULT NULL,

    CONSTRAINT workspace_connections_updated_after_created CHECK (updated_at >= created_at),
    CONSTRAINT workspace_connections_deleted_after_created CHECK (deleted_at IS NULL OR deleted_at >= created_at)
);

-- Triggers
SELECT setup_updated_at('workspace_connections');

-- Indexes
CREATE INDEX workspace_connections_workspace_idx
    ON workspace_connections (workspace_id, created_at DESC)
    WHERE deleted_at IS NULL;

CREATE INDEX workspace_connections_provider_idx
    ON workspace_connections (provider, workspace_id)
    WHERE deleted_at IS NULL;

CREATE UNIQUE INDEX workspace_connections_name_unique_idx
    ON workspace_connections (workspace_id, lower(trim(name)))
    WHERE deleted_at IS NULL;

CREATE INDEX workspace_connections_active_idx
    ON workspace_connections (workspace_id, is_active)
    WHERE deleted_at IS NULL AND is_active = TRUE;

-- Comments
COMMENT ON TABLE workspace_connections IS
    'Encrypted provider connections (credentials + context) scoped to workspaces.';

COMMENT ON COLUMN workspace_connections.id IS 'Unique connection identifier';
COMMENT ON COLUMN workspace_connections.workspace_id IS 'Parent workspace reference';
COMMENT ON COLUMN workspace_connections.account_id IS 'Creator account reference';
COMMENT ON COLUMN workspace_connections.name IS 'Human-readable connection name (1-255 chars)';
COMMENT ON COLUMN workspace_connections.provider IS 'Provider type (openai, postgres, s3, pinecone, etc.)';
COMMENT ON COLUMN workspace_connections.encrypted_data IS 'XChaCha20-Poly1305 encrypted JSON with credentials and context';
COMMENT ON COLUMN workspace_connections.is_active IS 'Whether the connection is active for syncing';
COMMENT ON COLUMN workspace_connections.last_sync_at IS 'Last successful sync timestamp';
COMMENT ON COLUMN workspace_connections.sync_status IS 'Current sync status';
COMMENT ON COLUMN workspace_connections.metadata IS 'Non-encrypted metadata for filtering/display';
COMMENT ON COLUMN workspace_connections.created_at IS 'Creation timestamp';
COMMENT ON COLUMN workspace_connections.updated_at IS 'Last modification timestamp';
COMMENT ON COLUMN workspace_connections.deleted_at IS 'Soft deletion timestamp';
