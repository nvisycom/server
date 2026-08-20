-- Connections: encrypted provider connections scoped to workspaces.
--
-- A connection is a generic, capability-agnostic credential holder: any
-- external provider (object store, LLM, ...) is a row here, distinguished only
-- by its `provider` and the shape of its encrypted config. Capabilities are
-- normalized into satellite tables — a connection that can sync has a
-- `workspace_connection_schedule` row and accrues `workspace_connection_syncs`
-- executions; a connection without those is just stored credentials.

-- Execution status of a connection sync.
CREATE TYPE SYNC_STATUS AS ENUM (
    'pending',      -- Sync is queued
    'running',      -- Sync is in progress
    'completed',    -- Sync finished successfully
    'failed',       -- Sync failed with error
    'cancelled'     -- Sync was cancelled
);

COMMENT ON TYPE SYNC_STATUS IS 'Execution status for connection syncs.';

-- How a connection sync was initiated.
CREATE TYPE SYNC_TRIGGER_TYPE AS ENUM (
    'manual',       -- Manually triggered by a user
    'scheduled',    -- Triggered by the connection's schedule
    'webhook'       -- Triggered by an inbound webhook
);

COMMENT ON TYPE SYNC_TRIGGER_TYPE IS 'How a connection sync was initiated.';

-- Direction a connection syncs data.
CREATE TYPE SYNC_MODE AS ENUM (
    'import',       -- Fetch objects from the connection into the workspace
    'export'        -- Push workspace files out to the connection
);

COMMENT ON TYPE SYNC_MODE IS 'Direction a connection syncs: import into, or export out of, the workspace.';

-- What an import does with a file whose source object no longer exists.
CREATE TYPE SYNC_DELETION_POLICY AS ENUM (
    'ignore',       -- Leave the imported file untouched (additive-only)
    'delete'        -- Soft-delete the file and remove its stored object
);

COMMENT ON TYPE SYNC_DELETION_POLICY IS 'How an import reconciles files whose source object has been deleted.';

-- Capability category of a connection's provider. Stable, closed set: the
-- concrete provider (`provider` column) stays open and extensible, but its
-- capability is one of these types. Lets a connection be found by what it can do
-- (e.g. the workspace's language model) without decrypting its config.
CREATE TYPE PROVIDER_TYPE AS ENUM (
    'object_store',     -- External object storage (s3, azure, gcs)
    'language_model'    -- LLM inference (openai, ollama, anthropic)
);

COMMENT ON TYPE PROVIDER_TYPE IS 'Capability category of a connection provider (object store, language model, ...).';

-- Workspace connections table: generic encrypted provider credentials.
CREATE TABLE workspace_connections (
    -- Primary identifier
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References
    workspace_id    UUID            NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    account_id      UUID            NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,

    -- Composite key target for workspace-scoped access and foreign keys.
    CONSTRAINT workspace_connections_workspace_id_id_key UNIQUE (workspace_id, id),

    -- Human-readable label for the connection.
    display_name    TEXT            NOT NULL,
    CONSTRAINT workspace_connections_display_name_length CHECK (length(trim(display_name)) BETWEEN 1 AND 255),

    -- The concrete provider (open, extensible: 's3', 'anthropic', ...) and its
    -- capability category (a stable, closed enum). The category lets a connection
    -- be found by what it can do without decrypting its config.
    provider        TEXT            NOT NULL,
    CONSTRAINT workspace_connections_provider_length CHECK (length(trim(provider)) BETWEEN 1 AND 64),
    provider_type   PROVIDER_TYPE   NOT NULL,

    -- Encrypted connection config (XChaCha20-Poly1305 encrypted JSON): the
    -- provider tag, credentials, and any provider-specific settings.
    encrypted_data  BYTEA           NOT NULL,
    CONSTRAINT workspace_connections_data_size CHECK (length(encrypted_data) BETWEEN 1 AND 65536),

    -- Whether the connection is enabled. For sync-capable connections this gates
    -- scheduled and manual syncs.
    is_active       BOOLEAN         NOT NULL DEFAULT TRUE,

    -- Non-encrypted metadata for filtering and display.
    metadata        JSONB           NOT NULL DEFAULT '{}',
    CONSTRAINT workspace_connections_metadata_size CHECK (length(metadata::TEXT) BETWEEN 2 AND 65536),

    -- Lifecycle timestamps
    created_at      TIMESTAMPTZ     NOT NULL DEFAULT current_timestamp,
    updated_at      TIMESTAMPTZ     NOT NULL DEFAULT current_timestamp,
    deleted_at      TIMESTAMPTZ     DEFAULT NULL,
    CONSTRAINT workspace_connections_updated_after_created CHECK (updated_at >= created_at),
    CONSTRAINT workspace_connections_deleted_after_created CHECK (deleted_at IS NULL OR deleted_at >= created_at)
);

-- Keep updated_at current on every write.
SELECT setup_updated_at('workspace_connections');

-- Most recent live connections per workspace (the connection list).
CREATE INDEX workspace_connections_workspace_idx
    ON workspace_connections (workspace_id, created_at DESC)
    WHERE deleted_at IS NULL;

-- Look up a workspace's connections for a given concrete provider.
CREATE INDEX workspace_connections_provider_idx
    ON workspace_connections (provider, workspace_id)
    WHERE deleted_at IS NULL;

-- Find a workspace's connection of a given capability (e.g. its language model),
-- most recently updated first.
CREATE INDEX workspace_connections_provider_type_idx
    ON workspace_connections (workspace_id, provider_type, updated_at DESC)
    WHERE deleted_at IS NULL;

-- Enforce a unique display name per workspace among live connections.
CREATE UNIQUE INDEX workspace_connections_display_name_unique_idx
    ON workspace_connections (workspace_id, lower(trim(display_name)))
    WHERE deleted_at IS NULL;

-- Live, enabled connections per workspace.
CREATE INDEX workspace_connections_active_idx
    ON workspace_connections (workspace_id, is_active)
    WHERE deleted_at IS NULL AND is_active = TRUE;

COMMENT ON TABLE workspace_connections IS 'Generic encrypted provider connections scoped to workspaces. Capabilities live in satellite tables.';
COMMENT ON COLUMN workspace_connections.id IS 'Unique connection identifier';
COMMENT ON COLUMN workspace_connections.workspace_id IS 'Workspace this connection belongs to';
COMMENT ON COLUMN workspace_connections.account_id IS 'Account that created the connection';
COMMENT ON COLUMN workspace_connections.display_name IS 'Human-readable connection display name (1-255 chars)';
COMMENT ON COLUMN workspace_connections.provider IS 'Concrete provider identifier (e.g. s3, azure, gcs, openai, ollama, anthropic)';
COMMENT ON COLUMN workspace_connections.provider_type IS 'Capability category of the provider (object_store, language_model)';
COMMENT ON COLUMN workspace_connections.encrypted_data IS 'XChaCha20-Poly1305 encrypted JSON: provider config + credentials';
COMMENT ON COLUMN workspace_connections.is_active IS 'Whether the connection is enabled';
COMMENT ON COLUMN workspace_connections.metadata IS 'Non-encrypted metadata for filtering/display';
COMMENT ON COLUMN workspace_connections.created_at IS 'Connection creation timestamp';
COMMENT ON COLUMN workspace_connections.updated_at IS 'Last modification timestamp';
COMMENT ON COLUMN workspace_connections.deleted_at IS 'Soft-deletion timestamp; NULL means live';

-- Connection schedule table (satellite): the sync capability's configuration.
-- Present only for connections that synchronize (e.g. object stores); its
-- presence is what makes a connection sync-capable.
CREATE TABLE workspace_connection_schedule (
    -- One schedule per connection; the connection id is the primary key.
    connection_id   UUID PRIMARY KEY REFERENCES workspace_connections (id) ON DELETE CASCADE,

    -- Whether the connection imports data in or exports data out.
    sync_mode       SYNC_MODE               NOT NULL DEFAULT 'import',

    -- Cron expression for scheduled syncs; NULL means manual-only.
    schedule_cron   TEXT                    DEFAULT NULL,
    CONSTRAINT workspace_connection_schedule_cron_length CHECK (schedule_cron IS NULL OR length(schedule_cron) BETWEEN 9 AND 100),

    -- What an import does when a source object it previously imported is gone.
    deletion_policy SYNC_DELETION_POLICY    NOT NULL DEFAULT 'ignore',

    -- Scheduled syncs are import-only for now; export is manual.
    CONSTRAINT workspace_connection_schedule_import_only CHECK (schedule_cron IS NULL OR sync_mode = 'import')
);

COMMENT ON TABLE workspace_connection_schedule IS 'Sync configuration for sync-capable connections. Its presence marks a connection as sync-capable.';
COMMENT ON COLUMN workspace_connection_schedule.connection_id IS 'The connection this schedule configures';
COMMENT ON COLUMN workspace_connection_schedule.sync_mode IS 'Whether the connection imports data in or exports data out';
COMMENT ON COLUMN workspace_connection_schedule.schedule_cron IS 'Cron expression for scheduled syncs; NULL means manual-only';
COMMENT ON COLUMN workspace_connection_schedule.deletion_policy IS 'How an import reconciles files whose source object was deleted';

-- Connection syncs table: one synchronization execution of a sync-capable
-- connection. References the schedule (not the bare connection), so a sync can
-- only exist for a connection that is sync-capable.
CREATE TABLE workspace_connection_syncs (
    -- Primary identifier
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References
    connection_id   UUID                NOT NULL REFERENCES workspace_connection_schedule (connection_id) ON DELETE CASCADE,
    account_id      UUID                NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,

    -- How the sync was initiated and where it currently stands.
    trigger_type    SYNC_TRIGGER_TYPE   NOT NULL DEFAULT 'manual',
    status          SYNC_STATUS         NOT NULL DEFAULT 'running',

    -- Number of records processed by this sync. An import lists the source and
    -- transfers only objects not already imported, so imports are incremental
    -- across invocations without any stored cursor; an export transfers workspace
    -- files out. Only imports can be scheduled (see the schedule table); exports
    -- are manual.
    records_synced  BIGINT              NOT NULL DEFAULT 0,
    CONSTRAINT workspace_connection_syncs_records_synced_non_negative CHECK (records_synced >= 0),

    -- 1-based attempt number for scheduled syncs; a failed scheduled sync may be
    -- re-enqueued up to a bounded number of attempts. Manual syncs are always 1.
    attempt         INTEGER             NOT NULL DEFAULT 1,
    CONSTRAINT workspace_connection_syncs_attempt_positive CHECK (attempt >= 1),

    -- Failure detail, populated when status is 'failed'.
    error_message   TEXT                DEFAULT NULL,
    CONSTRAINT workspace_connection_syncs_error_message_length CHECK (error_message IS NULL OR length(error_message) BETWEEN 1 AND 4096),

    -- Non-encrypted metadata for filtering and display.
    metadata        JSONB               NOT NULL DEFAULT '{}',
    CONSTRAINT workspace_connection_syncs_metadata_size CHECK (length(metadata::TEXT) BETWEEN 2 AND 65536),

    -- Timing
    started_at      TIMESTAMPTZ         NOT NULL DEFAULT current_timestamp,
    completed_at    TIMESTAMPTZ         DEFAULT NULL,
    CONSTRAINT workspace_connection_syncs_completed_after_started CHECK (completed_at IS NULL OR completed_at >= started_at)
);

-- A connection's sync history, most recent first.
CREATE INDEX workspace_connection_syncs_connection_idx
    ON workspace_connection_syncs (connection_id, started_at DESC);

-- An account's triggered syncs, most recent first.
CREATE INDEX workspace_connection_syncs_account_idx
    ON workspace_connection_syncs (account_id, started_at DESC);

-- Scan in-flight syncs (pending/running) across connections.
CREATE INDEX workspace_connection_syncs_status_idx
    ON workspace_connection_syncs (status, started_at DESC)
    WHERE status IN ('pending', 'running');

-- At most one active (pending/running) sync per connection. Enforces the
-- one-in-flight-sync invariant at the database level, closing the race between
-- checking for an in-flight sync and inserting a new one.
CREATE UNIQUE INDEX workspace_connection_syncs_one_active_idx
    ON workspace_connection_syncs (connection_id)
    WHERE status IN ('pending', 'running');

COMMENT ON TABLE workspace_connection_syncs IS 'Connection syncs: one synchronization execution of a connection, with progress and outcome.';
COMMENT ON COLUMN workspace_connection_syncs.id IS 'Unique sync identifier';
COMMENT ON COLUMN workspace_connection_syncs.connection_id IS 'Connection the sync synchronizes';
COMMENT ON COLUMN workspace_connection_syncs.account_id IS 'Account that triggered the sync';
COMMENT ON COLUMN workspace_connection_syncs.trigger_type IS 'How the sync was initiated';
COMMENT ON COLUMN workspace_connection_syncs.status IS 'Current sync status';
COMMENT ON COLUMN workspace_connection_syncs.records_synced IS 'Number of records processed';
COMMENT ON COLUMN workspace_connection_syncs.attempt IS '1-based attempt number; scheduled syncs may be retried up to a bounded limit';
COMMENT ON COLUMN workspace_connection_syncs.error_message IS 'Failure detail when status is failed';
COMMENT ON COLUMN workspace_connection_syncs.metadata IS 'Non-encrypted metadata for filtering/display';
COMMENT ON COLUMN workspace_connection_syncs.started_at IS 'When the sync started';
COMMENT ON COLUMN workspace_connection_syncs.completed_at IS 'When the sync finished';

-- File imports table (satellite): for a file that was imported from a
-- connection, the connection and remote object key it came from. Present only
-- for imported files; uploaded/generated files have no row here. Lives here
-- rather than on `workspace_files` because the FK target (`workspace_connections`)
-- is created by this migration, which runs after the files migration.
CREATE TABLE workspace_file_imports (
    -- One import origin per file; the file id is the primary key.
    file_id         UUID PRIMARY KEY REFERENCES workspace_files (id) ON DELETE CASCADE,

    -- The connection and remote object key the file was imported from. Deleting
    -- the connection drops the import origin rows (the files themselves remain).
    connection_id   UUID                NOT NULL REFERENCES workspace_connections (id) ON DELETE CASCADE,
    source_key      TEXT                NOT NULL,
    CONSTRAINT workspace_file_imports_source_key_length CHECK (length(source_key) BETWEEN 1 AND 1024)
);

-- One imported file per (connection, remote key): makes re-imports idempotent
-- and backs the "already imported" lookup during sync.
CREATE UNIQUE INDEX workspace_file_imports_source_object_unique_idx
    ON workspace_file_imports (connection_id, source_key);

COMMENT ON TABLE workspace_file_imports IS 'Import origin for imported files: the connection and remote object key each came from.';
COMMENT ON COLUMN workspace_file_imports.file_id IS 'The imported file this origin describes';
COMMENT ON COLUMN workspace_file_imports.connection_id IS 'Connection the file was imported from';
COMMENT ON COLUMN workspace_file_imports.source_key IS 'Remote object key the file was imported from';
