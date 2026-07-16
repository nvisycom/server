-- Webhooks: per-workspace outbound webhook subscriptions and delivery
-- status. Workspace-scoped but a standalone integration feature.

-- Webhook status enum
CREATE TYPE WEBHOOK_STATUS AS ENUM (
    'active',       -- Webhook is active and will receive events
    'paused',       -- Webhook is temporarily paused
    'disabled'      -- Webhook is disabled
);

COMMENT ON TYPE WEBHOOK_STATUS IS
    'Defines the operational status of workspace webhooks.';

-- Webhook event types enum
CREATE TYPE WEBHOOK_EVENT AS ENUM (
    -- File events
    'file:created',
    'file:updated',
    'file:deleted',

    -- Member events
    'member:added',
    'member:deleted',
    'member:updated',

    -- Connection events
    'connection:created',
    'connection:updated',
    'connection:deleted',
    'connection:synced',
    'connection:desynced'
);

COMMENT ON TYPE WEBHOOK_EVENT IS
    'Defines the types of events that can trigger webhook delivery.';

-- Workspace webhooks table definition
CREATE TABLE workspace_webhooks (
    -- Primary identifier
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Reference
    workspace_id     UUID             NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,

    -- Composite key target for workspace-scoped access and foreign keys.
    CONSTRAINT workspace_webhooks_workspace_id_id_key UNIQUE (workspace_id, id),

    -- URL identity, unique within the workspace: lowercase alphanumeric with
    -- single internal dashes, 3-32 characters.
    slug             TEXT             NOT NULL,
    CONSTRAINT workspace_webhooks_workspace_id_slug_key UNIQUE (workspace_id, slug),
    CONSTRAINT workspace_webhooks_slug_length CHECK (length(slug) BETWEEN 3 AND 32),
    CONSTRAINT workspace_webhooks_slug_format CHECK (slug ~ '^[a-z0-9]+(-[a-z0-9]+)*$'),

    -- Webhook details
    display_name     TEXT             NOT NULL,
    description      TEXT             NOT NULL DEFAULT '',
    url              TEXT             NOT NULL,

    CONSTRAINT workspace_webhooks_display_name_length CHECK (length(trim(display_name)) BETWEEN 1 AND 128),
    CONSTRAINT workspace_webhooks_description_length CHECK (length(description) <= 500),
    CONSTRAINT workspace_webhooks_url_length CHECK (length(url) BETWEEN 10 AND 2048),
    CONSTRAINT workspace_webhooks_url_format CHECK (url ~ '^https?://'),

    -- Event configuration
    events           WEBHOOK_EVENT[]  NOT NULL DEFAULT '{}',
    headers          JSONB            NOT NULL DEFAULT '{}',

    -- HMAC signing secret, XChaCha20-Poly1305 encrypted under the workspace key.
    -- Generated and returned to the caller once at creation; the server decrypts
    -- it to sign each delivery.
    encrypted_secret BYTEA            NOT NULL,

    CONSTRAINT workspace_webhooks_events_not_empty CHECK (array_length(events, 1) > 0),
    CONSTRAINT workspace_webhooks_headers_size CHECK (length(headers::TEXT) BETWEEN 2 AND 4096),

    -- Webhook status
    status           WEBHOOK_STATUS   NOT NULL DEFAULT 'active',
    last_triggered_at TIMESTAMPTZ     DEFAULT NULL,

    -- Audit tracking
    created_by       UUID             NOT NULL REFERENCES accounts (id),

    -- Lifecycle timestamps
    created_at       TIMESTAMPTZ      NOT NULL DEFAULT current_timestamp,
    updated_at       TIMESTAMPTZ      NOT NULL DEFAULT current_timestamp,
    deleted_at       TIMESTAMPTZ      DEFAULT NULL,

    CONSTRAINT workspace_webhooks_updated_after_created CHECK (updated_at >= created_at),
    CONSTRAINT workspace_webhooks_deleted_after_created CHECK (deleted_at IS NULL OR deleted_at >= created_at)
);

-- Triggers for workspace_webhooks table
SELECT setup_updated_at('workspace_webhooks');

-- Indexes for workspace_webhooks table
CREATE INDEX workspace_webhooks_workspace_status_idx
    ON workspace_webhooks (workspace_id, status)
    WHERE deleted_at IS NULL;

CREATE INDEX workspace_webhooks_events_idx
    ON workspace_webhooks USING gin (events)
    WHERE deleted_at IS NULL AND status = 'active';

-- Comments for workspace_webhooks table
COMMENT ON TABLE workspace_webhooks IS
    'Webhook configurations for workspaces to receive event notifications.';

COMMENT ON COLUMN workspace_webhooks.id IS 'Unique webhook identifier';
COMMENT ON COLUMN workspace_webhooks.workspace_id IS 'Reference to the workspace';
COMMENT ON COLUMN workspace_webhooks.display_name IS 'Human-readable webhook name (1-128 chars)';
COMMENT ON COLUMN workspace_webhooks.description IS 'Webhook description (up to 500 chars)';
COMMENT ON COLUMN workspace_webhooks.url IS 'Webhook endpoint URL (must be HTTP/HTTPS)';
COMMENT ON COLUMN workspace_webhooks.events IS 'Array of event types this webhook subscribes to';
COMMENT ON COLUMN workspace_webhooks.headers IS 'Custom headers to include in webhook requests';
COMMENT ON COLUMN workspace_webhooks.status IS 'Current webhook status (active, paused, disabled)';
COMMENT ON COLUMN workspace_webhooks.last_triggered_at IS 'Timestamp of last webhook trigger';
COMMENT ON COLUMN workspace_webhooks.created_by IS 'Account that created the webhook';
COMMENT ON COLUMN workspace_webhooks.created_at IS 'Timestamp when webhook was created';
COMMENT ON COLUMN workspace_webhooks.updated_at IS 'Timestamp when webhook was last modified';
COMMENT ON COLUMN workspace_webhooks.deleted_at IS 'Soft deletion timestamp';

