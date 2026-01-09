-- Studio: LLM-powered document editing sessions and operations tracking

-- Studio session lifecycle status
CREATE TYPE STUDIO_SESSION_STATUS AS ENUM (
    'active',
    'paused',
    'archived'
);

COMMENT ON TYPE STUDIO_SESSION_STATUS IS
    'Lifecycle status for studio editing sessions.';

-- Studio sessions table definition
CREATE TABLE studio_sessions (
    -- Primary identifier
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References
    workspace_id        UUID                    NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    account_id          UUID                    NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    primary_file_id     UUID                    NOT NULL REFERENCES document_files (id) ON DELETE CASCADE,

    -- Session attributes
    display_name        TEXT                    NOT NULL DEFAULT 'Untitled Session',
    session_status      STUDIO_SESSION_STATUS   NOT NULL DEFAULT 'active',

    CONSTRAINT studio_sessions_display_name_length CHECK (length(trim(display_name)) BETWEEN 1 AND 255),

    -- Model configuration (model name, temperature, max tokens, etc.)
    model_config        JSONB                   NOT NULL DEFAULT '{}',

    CONSTRAINT studio_sessions_model_config_size CHECK (length(model_config::TEXT) BETWEEN 2 AND 8192),

    -- Usage statistics
    message_count       INTEGER                 NOT NULL DEFAULT 0,
    token_count         INTEGER                 NOT NULL DEFAULT 0,

    CONSTRAINT studio_sessions_message_count_min CHECK (message_count >= 0),
    CONSTRAINT studio_sessions_token_count_min CHECK (token_count >= 0),

    -- Lifecycle timestamps
    created_at          TIMESTAMPTZ             NOT NULL DEFAULT current_timestamp,
    updated_at          TIMESTAMPTZ             NOT NULL DEFAULT current_timestamp,

    CONSTRAINT studio_sessions_updated_after_created CHECK (updated_at >= created_at)
);

-- Triggers for studio_sessions table
SELECT setup_updated_at('studio_sessions');

-- Indexes for studio_sessions table
CREATE INDEX studio_sessions_workspace_idx
    ON studio_sessions (workspace_id, created_at DESC);

CREATE INDEX studio_sessions_account_idx
    ON studio_sessions (account_id, created_at DESC);

CREATE INDEX studio_sessions_file_idx
    ON studio_sessions (primary_file_id);

CREATE INDEX studio_sessions_status_idx
    ON studio_sessions (session_status, workspace_id)
    WHERE session_status = 'active';

-- Comments for studio_sessions table
COMMENT ON TABLE studio_sessions IS
    'LLM-assisted document editing sessions.';

COMMENT ON COLUMN studio_sessions.id IS 'Unique session identifier';
COMMENT ON COLUMN studio_sessions.workspace_id IS 'Reference to the workspace';
COMMENT ON COLUMN studio_sessions.account_id IS 'Account that created the session';
COMMENT ON COLUMN studio_sessions.primary_file_id IS 'Primary file being edited in this session';
COMMENT ON COLUMN studio_sessions.display_name IS 'User-friendly session name (1-255 chars)';
COMMENT ON COLUMN studio_sessions.session_status IS 'Session lifecycle status (active, paused, archived)';
COMMENT ON COLUMN studio_sessions.model_config IS 'LLM configuration (model, temperature, etc.)';
COMMENT ON COLUMN studio_sessions.message_count IS 'Total number of messages exchanged in this session';
COMMENT ON COLUMN studio_sessions.token_count IS 'Total tokens used in this session';
COMMENT ON COLUMN studio_sessions.created_at IS 'Timestamp when session was created';
COMMENT ON COLUMN studio_sessions.updated_at IS 'Timestamp when session was last modified';

-- Tool execution status
CREATE TYPE STUDIO_TOOL_STATUS AS ENUM (
    'pending',
    'running',
    'completed',
    'cancelled'
);

COMMENT ON TYPE STUDIO_TOOL_STATUS IS
    'Execution status for studio tool calls.';

-- Studio tool calls table definition
CREATE TABLE studio_tool_calls (
    -- Primary identifier
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References
    session_id          UUID                    NOT NULL REFERENCES studio_sessions (id) ON DELETE CASCADE,
    file_id             UUID                    NOT NULL REFERENCES document_files (id) ON DELETE CASCADE,
    chunk_id            UUID                    DEFAULT NULL REFERENCES document_chunks (id) ON DELETE SET NULL,

    -- Tool attributes
    tool_name           TEXT                    NOT NULL,
    tool_input          JSONB                   NOT NULL DEFAULT '{}',
    tool_output         JSONB                   NOT NULL DEFAULT '{}',
    tool_status         STUDIO_TOOL_STATUS      NOT NULL DEFAULT 'pending',

    CONSTRAINT studio_tool_calls_tool_name_length CHECK (length(trim(tool_name)) BETWEEN 1 AND 128),
    CONSTRAINT studio_tool_calls_tool_input_size CHECK (length(tool_input::TEXT) BETWEEN 2 AND 65536),
    CONSTRAINT studio_tool_calls_tool_output_size CHECK (length(tool_output::TEXT) BETWEEN 2 AND 65536),

    -- Timing
    started_at          TIMESTAMPTZ             NOT NULL DEFAULT current_timestamp,
    completed_at        TIMESTAMPTZ             DEFAULT NULL,

    CONSTRAINT studio_tool_calls_completed_after_started CHECK (completed_at IS NULL OR completed_at >= started_at)
);

-- Indexes for studio_tool_calls table
CREATE INDEX studio_tool_calls_session_idx
    ON studio_tool_calls (session_id, started_at DESC);

CREATE INDEX studio_tool_calls_file_idx
    ON studio_tool_calls (file_id, started_at DESC);

CREATE INDEX studio_tool_calls_status_idx
    ON studio_tool_calls (tool_status, started_at DESC)
    WHERE tool_status IN ('pending', 'running');

CREATE INDEX studio_tool_calls_tool_name_idx
    ON studio_tool_calls (tool_name);

-- Comments for studio_tool_calls table
COMMENT ON TABLE studio_tool_calls IS
    'Tool invocations for debugging and usage tracking. Input/output contain references, not document content.';

COMMENT ON COLUMN studio_tool_calls.id IS 'Unique tool call identifier';
COMMENT ON COLUMN studio_tool_calls.session_id IS 'Reference to the studio session';
COMMENT ON COLUMN studio_tool_calls.file_id IS 'Reference to the file being operated on';
COMMENT ON COLUMN studio_tool_calls.chunk_id IS 'Optional reference to a specific chunk';
COMMENT ON COLUMN studio_tool_calls.tool_name IS 'Name of the tool (merge, split, redact, translate, etc.)';
COMMENT ON COLUMN studio_tool_calls.tool_input IS 'Tool parameters as JSON (references, not content)';
COMMENT ON COLUMN studio_tool_calls.tool_output IS 'Tool result as JSON (references, not content)';
COMMENT ON COLUMN studio_tool_calls.tool_status IS 'Execution status (pending, running, completed, cancelled)';
COMMENT ON COLUMN studio_tool_calls.started_at IS 'Timestamp when tool call was created/started';
COMMENT ON COLUMN studio_tool_calls.completed_at IS 'Timestamp when tool execution completed';

-- Studio operations table definition
CREATE TABLE studio_operations (
    -- Primary identifier
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References
    tool_call_id        UUID                    NOT NULL REFERENCES studio_tool_calls (id) ON DELETE CASCADE,
    file_id             UUID                    NOT NULL REFERENCES document_files (id) ON DELETE CASCADE,
    chunk_id            UUID                    DEFAULT NULL REFERENCES document_chunks (id) ON DELETE SET NULL,

    -- Operation attributes
    operation_type      TEXT                    NOT NULL,
    operation_diff      JSONB                   NOT NULL DEFAULT '{}',

    CONSTRAINT studio_operations_operation_type_length CHECK (length(trim(operation_type)) BETWEEN 1 AND 64),
    CONSTRAINT studio_operations_operation_diff_size CHECK (length(operation_diff::TEXT) BETWEEN 2 AND 131072),

    -- Application state
    applied             BOOLEAN                 NOT NULL DEFAULT FALSE,
    reverted            BOOLEAN                 NOT NULL DEFAULT FALSE,

    CONSTRAINT studio_operations_revert_requires_applied CHECK (NOT reverted OR applied),

    -- Timing
    created_at          TIMESTAMPTZ             NOT NULL DEFAULT current_timestamp,
    applied_at          TIMESTAMPTZ             DEFAULT NULL,

    CONSTRAINT studio_operations_applied_after_created CHECK (applied_at IS NULL OR applied_at >= created_at)
);

-- Indexes for studio_operations table
CREATE INDEX studio_operations_tool_call_idx
    ON studio_operations (tool_call_id);

CREATE INDEX studio_operations_file_idx
    ON studio_operations (file_id, created_at DESC);

CREATE INDEX studio_operations_pending_idx
    ON studio_operations (file_id, applied)
    WHERE NOT applied;

-- Comments for studio_operations table
COMMENT ON TABLE studio_operations IS
    'Document operations (diffs) produced by tool calls. Stores positions, not content.';

COMMENT ON COLUMN studio_operations.id IS 'Unique operation identifier';
COMMENT ON COLUMN studio_operations.tool_call_id IS 'Reference to the tool call that produced this operation';
COMMENT ON COLUMN studio_operations.file_id IS 'Reference to the file being modified';
COMMENT ON COLUMN studio_operations.chunk_id IS 'Optional reference to a specific chunk';
COMMENT ON COLUMN studio_operations.operation_type IS 'Type of operation (insert, replace, delete, format, merge, split, etc.)';
COMMENT ON COLUMN studio_operations.operation_diff IS 'The diff specification as JSON (positions, not content)';
COMMENT ON COLUMN studio_operations.applied IS 'Whether this operation has been applied to the document';
COMMENT ON COLUMN studio_operations.reverted IS 'Whether this operation was reverted by the user';
COMMENT ON COLUMN studio_operations.created_at IS 'Timestamp when operation was created';
COMMENT ON COLUMN studio_operations.applied_at IS 'Timestamp when operation was applied';
