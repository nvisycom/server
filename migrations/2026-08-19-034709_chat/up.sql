-- Chat: workspace-scoped assistant conversations. A standalone workspace
-- resource. Each session is a thread of messages; the assistant's replies are
-- produced by the workspace's inference connection.

-- Role of a chat message: who authored it.
CREATE TYPE CHAT_ROLE AS ENUM (
    'system',       -- System instruction (server-authored context)
    'user',         -- A message from the account
    'assistant'     -- A reply from the model
);

COMMENT ON TYPE CHAT_ROLE IS 'Author of a chat message: system, user, or assistant.';

-- Chat sessions table: one conversation thread within a workspace.
CREATE TABLE chat_sessions (
    -- Primary identifier
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References
    workspace_id    UUID            NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    account_id      UUID            NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,

    -- Human-readable title, seeded from the first message (editable).
    title           TEXT            NOT NULL DEFAULT 'New chat',
    CONSTRAINT chat_sessions_title_length CHECK (length(trim(title)) BETWEEN 1 AND 255),

    -- Lifecycle timestamps
    created_at      TIMESTAMPTZ     NOT NULL DEFAULT current_timestamp,
    updated_at      TIMESTAMPTZ     NOT NULL DEFAULT current_timestamp,
    deleted_at      TIMESTAMPTZ     DEFAULT NULL
);

-- Most recent live sessions per workspace (the session list).
CREATE INDEX chat_sessions_workspace_recent_idx
    ON chat_sessions (workspace_id, updated_at DESC)
    WHERE deleted_at IS NULL;

COMMENT ON TABLE chat_sessions IS 'Workspace-scoped assistant conversation threads.';
COMMENT ON COLUMN chat_sessions.id IS 'Unique session identifier';
COMMENT ON COLUMN chat_sessions.workspace_id IS 'Workspace this session belongs to';
COMMENT ON COLUMN chat_sessions.account_id IS 'Account that opened the session';
COMMENT ON COLUMN chat_sessions.title IS 'Human-readable title (seeded from the first message)';
COMMENT ON COLUMN chat_sessions.created_at IS 'Session creation timestamp';
COMMENT ON COLUMN chat_sessions.updated_at IS 'Timestamp of the most recent message';
COMMENT ON COLUMN chat_sessions.deleted_at IS 'Soft-deletion timestamp; NULL means live';

-- Chat messages table: the ordered turns of a session.
CREATE TABLE chat_messages (
    -- Primary identifier
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References
    session_id      UUID            NOT NULL REFERENCES chat_sessions (id) ON DELETE CASCADE,

    -- Message details. The content is stored XChaCha20-Poly1305 encrypted with
    -- the workspace-derived key (a user may paste sensitive text into the
    -- assistant), so it is opaque bytes rather than searchable text.
    role            CHAT_ROLE       NOT NULL,
    content         BYTEA           NOT NULL,
    CONSTRAINT chat_messages_content_size CHECK (length(content) BETWEEN 1 AND 131072),

    -- Lifecycle timestamp
    created_at      TIMESTAMPTZ     NOT NULL DEFAULT current_timestamp
);

-- Ordered history of a session (oldest first when read).
CREATE INDEX chat_messages_session_created_idx
    ON chat_messages (session_id, created_at);

COMMENT ON TABLE chat_messages IS 'Ordered messages of a chat session.';
COMMENT ON COLUMN chat_messages.id IS 'Unique message identifier';
COMMENT ON COLUMN chat_messages.session_id IS 'Session this message belongs to';
COMMENT ON COLUMN chat_messages.role IS 'Author of the message (user or assistant)';
COMMENT ON COLUMN chat_messages.content IS 'XChaCha20-Poly1305 encrypted message text';
COMMENT ON COLUMN chat_messages.created_at IS 'Message creation timestamp';
