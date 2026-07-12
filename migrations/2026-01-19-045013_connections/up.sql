-- Connections: encrypted provider connections scoped to workspaces.

-- Sync status enum for connection sync runs
CREATE TYPE SYNC_STATUS AS ENUM (
    'pending',      -- Sync is queued
    'running',      -- Sync is in progress
    'completed',    -- Sync finished successfully
    'failed',       -- Sync failed with error
    'cancelled'     -- Sync was cancelled
);

COMMENT ON TYPE SYNC_STATUS IS
    'Execution status for connection sync runs.';

-- How a connection sync run was initiated
CREATE TYPE SYNC_TRIGGER_TYPE AS ENUM (
    'manual',       -- Manually triggered by user
    'scheduled',    -- Triggered by schedule
    'webhook'       -- Triggered by an inbound webhook
);

COMMENT ON TYPE SYNC_TRIGGER_TYPE IS
    'How a connection sync run was initiated.';

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

    -- Whether the connection is enabled for syncing. Current sync state and
    -- last-sync time are not stored here; they derive from the connection's
    -- sync runs (see workspace_connection_runs and the sync-state view).
    is_active       BOOLEAN         NOT NULL DEFAULT TRUE,

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
COMMENT ON COLUMN workspace_connections.is_active IS 'Whether the connection is enabled for syncing';
COMMENT ON COLUMN workspace_connections.metadata IS 'Non-encrypted metadata for filtering/display';
COMMENT ON COLUMN workspace_connections.created_at IS 'Creation timestamp';
COMMENT ON COLUMN workspace_connections.updated_at IS 'Last modification timestamp';
COMMENT ON COLUMN workspace_connections.deleted_at IS 'Soft deletion timestamp';

-- Connection sync runs table (one row per sync execution)
CREATE TABLE workspace_connection_runs (
    -- Primary identifier
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References
    connection_id   UUID                NOT NULL REFERENCES workspace_connections (id) ON DELETE CASCADE,
    account_id      UUID                REFERENCES accounts (id) ON DELETE SET NULL,

    -- Run attributes
    trigger_type    SYNC_TRIGGER_TYPE   NOT NULL DEFAULT 'manual',
    status          SYNC_STATUS         NOT NULL DEFAULT 'running',

    -- Number of records processed by this run. Resumption state (cursor,
    -- offset) is not stored here; it lives in the connection's encrypted
    -- context, which each run reads and advances.
    records_synced  BIGINT              NOT NULL DEFAULT 0,

    CONSTRAINT workspace_connection_runs_records_synced_non_negative CHECK (records_synced >= 0),

    -- Failure detail, populated when status is 'failed'.
    error_message   TEXT                DEFAULT NULL,

    CONSTRAINT workspace_connection_runs_error_message_length CHECK (error_message IS NULL OR length(error_message) BETWEEN 1 AND 4096),

    -- Metadata (non-encrypted, for filtering/display)
    metadata        JSONB               NOT NULL DEFAULT '{}',

    CONSTRAINT workspace_connection_runs_metadata_size CHECK (length(metadata::TEXT) BETWEEN 2 AND 65536),

    -- Timing
    started_at      TIMESTAMPTZ         NOT NULL DEFAULT current_timestamp,
    completed_at    TIMESTAMPTZ         DEFAULT NULL,

    CONSTRAINT workspace_connection_runs_completed_after_started CHECK (completed_at IS NULL OR completed_at >= started_at)
);

-- Indexes
CREATE INDEX workspace_connection_runs_connection_idx
    ON workspace_connection_runs (connection_id, started_at DESC);

CREATE INDEX workspace_connection_runs_account_idx
    ON workspace_connection_runs (account_id, started_at DESC)
    WHERE account_id IS NOT NULL;

CREATE INDEX workspace_connection_runs_status_idx
    ON workspace_connection_runs (status, started_at DESC)
    WHERE status IN ('pending', 'running');

-- Comments
COMMENT ON TABLE workspace_connection_runs IS
    'Sync runs: one synchronization execution of a connection, with progress and outcome.';

COMMENT ON COLUMN workspace_connection_runs.id IS 'Unique sync run identifier';
COMMENT ON COLUMN workspace_connection_runs.connection_id IS 'Connection the run synchronizes';
COMMENT ON COLUMN workspace_connection_runs.account_id IS 'Account that triggered the run (optional)';
COMMENT ON COLUMN workspace_connection_runs.trigger_type IS 'How the run was initiated';
COMMENT ON COLUMN workspace_connection_runs.status IS 'Current run status';
COMMENT ON COLUMN workspace_connection_runs.records_synced IS 'Number of records processed';
COMMENT ON COLUMN workspace_connection_runs.error_message IS 'Failure detail when status is failed';
COMMENT ON COLUMN workspace_connection_runs.metadata IS 'Non-encrypted metadata for filtering/display';
COMMENT ON COLUMN workspace_connection_runs.started_at IS 'When the run started';
COMMENT ON COLUMN workspace_connection_runs.completed_at IS 'When the run finished';

-- Current sync state per connection, derived from its most recent run.
-- Replaces denormalized status/timestamp columns on workspace_connections.
CREATE VIEW workspace_connection_sync_state AS
SELECT DISTINCT ON (cr.connection_id)
    cr.connection_id,
    c.workspace_id,
    cr.id                AS latest_run_id,
    cr.status,
    cr.trigger_type,
    cr.records_synced,
    cr.started_at        AS latest_started_at,
    cr.completed_at      AS latest_completed_at
FROM workspace_connection_runs cr
    JOIN workspace_connections c ON cr.connection_id = c.id
ORDER BY cr.connection_id, cr.started_at DESC;

COMMENT ON VIEW workspace_connection_sync_state IS
    'Latest sync run per connection, used as the connection''s current sync state.';
