-- Providers: encrypted inference-provider credentials scoped to workspaces.
--
-- A provider is an inference service the platform calls (a language model for
-- chat, a named-entity-recognition model for extraction) — a separate resource
-- from a connection: no transfers, no syncs, no schedule. Distinguished by its
-- `provider` (concrete vendor) and `provider_type` (model kind), the latter found
-- without decrypting the config.

-- Inference model type backing a workspace provider. The concrete vendor
-- (`provider` column) stays open; this is the model kind, used to find a
-- workspace's provider of a given type without decrypting.
CREATE TYPE PROVIDER_TYPE AS ENUM (
    'llm',      -- A language model (openai, ollama, anthropic)
    'ner'       -- A named-entity-recognition model
);

COMMENT ON TYPE PROVIDER_TYPE IS 'Inference model type of a workspace provider (llm, ner).';

CREATE TABLE workspace_providers (
    -- Primary identifier
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References
    workspace_id    UUID            NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    account_id      UUID            NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,

    -- Composite key target for workspace-scoped access and foreign keys.
    CONSTRAINT workspace_providers_workspace_id_id_key UNIQUE (workspace_id, id),

    -- Human-readable label for the provider.
    display_name    TEXT            NOT NULL,
    CONSTRAINT workspace_providers_display_name_length CHECK (length(trim(display_name)) BETWEEN 1 AND 255),

    -- The concrete provider (open, extensible: 'openai', 'ollama', ...) and its
    -- model type (a stable, closed enum). The type lets a workspace's provider of
    -- a given kind be found without decrypting its config.
    provider        TEXT            NOT NULL,
    CONSTRAINT workspace_providers_provider_length CHECK (length(trim(provider)) BETWEEN 1 AND 64),
    provider_type   PROVIDER_TYPE   NOT NULL,

    -- Encrypted provider config (XChaCha20-Poly1305 encrypted JSON): the provider
    -- tag, credentials, and any provider-specific settings.
    encrypted_data  BYTEA           NOT NULL,
    CONSTRAINT workspace_providers_data_size CHECK (length(encrypted_data) BETWEEN 1 AND 65536),

    -- Whether the provider is enabled.
    is_active       BOOLEAN         NOT NULL DEFAULT TRUE,

    -- Non-encrypted metadata for filtering and display.
    metadata        JSONB           NOT NULL DEFAULT '{}',
    CONSTRAINT workspace_providers_metadata_size CHECK (length(metadata::TEXT) BETWEEN 2 AND 65536),

    -- Lifecycle timestamps
    created_at      TIMESTAMPTZ     NOT NULL DEFAULT current_timestamp,
    updated_at      TIMESTAMPTZ     NOT NULL DEFAULT current_timestamp,
    deleted_at      TIMESTAMPTZ     DEFAULT NULL,
    CONSTRAINT workspace_providers_updated_after_created CHECK (updated_at >= created_at),
    CONSTRAINT workspace_providers_deleted_after_created CHECK (deleted_at IS NULL OR deleted_at >= created_at)
);

-- Keep updated_at current on every write.
SELECT setup_updated_at('workspace_providers');

-- Most recent live providers per workspace (the provider list).
CREATE INDEX workspace_providers_workspace_idx
    ON workspace_providers (workspace_id, created_at DESC)
    WHERE deleted_at IS NULL;

-- Look up a workspace's providers for a given concrete provider.
CREATE INDEX workspace_providers_provider_idx
    ON workspace_providers (provider, workspace_id)
    WHERE deleted_at IS NULL;

-- Find a workspace's provider of a given model type (e.g. its LLM), most recently
-- updated first.
CREATE INDEX workspace_providers_provider_type_idx
    ON workspace_providers (workspace_id, provider_type, updated_at DESC)
    WHERE deleted_at IS NULL;

-- Enforce a unique display name per workspace among live providers (own namespace,
-- independent of connections).
CREATE UNIQUE INDEX workspace_providers_display_name_unique_idx
    ON workspace_providers (workspace_id, lower(trim(display_name)))
    WHERE deleted_at IS NULL;

-- Live, enabled providers per workspace.
CREATE INDEX workspace_providers_active_idx
    ON workspace_providers (workspace_id, is_active)
    WHERE deleted_at IS NULL AND is_active = TRUE;

COMMENT ON TABLE workspace_providers IS 'Encrypted inference-provider credentials scoped to workspaces (LLM, NER). A separate resource from connections; no syncs.';
COMMENT ON COLUMN workspace_providers.id IS 'Unique provider identifier';
COMMENT ON COLUMN workspace_providers.workspace_id IS 'Workspace this provider belongs to';
COMMENT ON COLUMN workspace_providers.account_id IS 'Account that created the provider';
COMMENT ON COLUMN workspace_providers.display_name IS 'Human-readable provider display name (1-255 chars)';
COMMENT ON COLUMN workspace_providers.provider IS 'Concrete provider identifier (e.g. openai, ollama, anthropic)';
COMMENT ON COLUMN workspace_providers.provider_type IS 'Inference model type of the provider (llm, ner)';
COMMENT ON COLUMN workspace_providers.encrypted_data IS 'XChaCha20-Poly1305 encrypted JSON: provider config + credentials';
COMMENT ON COLUMN workspace_providers.is_active IS 'Whether the provider is enabled';
COMMENT ON COLUMN workspace_providers.metadata IS 'Non-encrypted metadata for filtering/display';
COMMENT ON COLUMN workspace_providers.created_at IS 'Provider creation timestamp';
COMMENT ON COLUMN workspace_providers.updated_at IS 'Last modification timestamp';
COMMENT ON COLUMN workspace_providers.deleted_at IS 'Soft-deletion timestamp; NULL means live';
