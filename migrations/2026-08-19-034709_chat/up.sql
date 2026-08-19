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

    -- The active leaf of the message tree: the message this conversation
    -- currently ends at. A client resumes from here, and a new turn without an
    -- explicit parent extends this. The FK is added after chat_messages exists.
    current_message_id UUID          DEFAULT NULL,

    -- Lifecycle timestamps
    created_at      TIMESTAMPTZ     NOT NULL DEFAULT current_timestamp,
    updated_at      TIMESTAMPTZ     NOT NULL DEFAULT current_timestamp,
    deleted_at      TIMESTAMPTZ     DEFAULT NULL,
    CONSTRAINT chat_sessions_updated_after_created CHECK (updated_at >= created_at),
    CONSTRAINT chat_sessions_deleted_after_created CHECK (deleted_at IS NULL OR deleted_at >= created_at),
    CONSTRAINT chat_sessions_deleted_after_updated CHECK (deleted_at IS NULL OR deleted_at >= updated_at)
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
COMMENT ON COLUMN chat_sessions.current_message_id IS 'Active leaf of the message tree (resume point)';
COMMENT ON COLUMN chat_sessions.created_at IS 'Session creation timestamp';
COMMENT ON COLUMN chat_sessions.updated_at IS 'Timestamp of the most recent message';
COMMENT ON COLUMN chat_sessions.deleted_at IS 'Soft-deletion timestamp; NULL means live';

-- Chat messages table: the conversation tree of a session.
CREATE TABLE chat_messages (
    -- Primary identifier
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References
    session_id      UUID            NOT NULL REFERENCES chat_sessions (id) ON DELETE CASCADE,

    -- Composite key target: lets the tree/leaf foreign keys pin a message to a
    -- specific session, so a parent (or a session's active leaf) can never point
    -- at a message from another session.
    CONSTRAINT chat_messages_id_session_key UNIQUE (id, session_id),

    -- The message this one replies to (its parent in the conversation tree).
    -- NULL is a root. A regenerated reply is a sibling: another child of the same
    -- parent. The active conversation is the path from a leaf back to the root.
    -- The composite FK enforces that a parent is in the same session.
    parent_id       UUID            DEFAULT NULL,
    CONSTRAINT chat_messages_parent_fkey
        FOREIGN KEY (parent_id, session_id)
        REFERENCES chat_messages (id, session_id) ON DELETE CASCADE,

    -- Message details. The content is stored XChaCha20-Poly1305 encrypted with
    -- the workspace-derived key (a user may paste sensitive text into the
    -- assistant), so it is opaque bytes rather than searchable text.
    role            CHAT_ROLE       NOT NULL,
    content         BYTEA           NOT NULL,
    CONSTRAINT chat_messages_content_size CHECK (length(content) BETWEEN 1 AND 131072),

    -- Lifecycle timestamp
    created_at      TIMESTAMPTZ     NOT NULL DEFAULT current_timestamp
);

-- All of a session's messages (the whole tree; the path is walked in-app).
CREATE INDEX chat_messages_session_idx
    ON chat_messages (session_id);

-- Walk a node's children (sibling branches), and back the parent composite FK.
CREATE INDEX chat_messages_parent_idx
    ON chat_messages (parent_id, session_id)
    WHERE parent_id IS NOT NULL;

-- The active-leaf pointer references a message in THIS session: the composite FK
-- ties the session's own id to the referenced message's session_id. Added now
-- that chat_messages exists. A deleted leaf clears the pointer rather than
-- cascading the session.
ALTER TABLE chat_sessions
    ADD CONSTRAINT chat_sessions_current_message_fkey
    FOREIGN KEY (current_message_id, id)
    REFERENCES chat_messages (id, session_id)
    ON DELETE SET NULL (current_message_id);

COMMENT ON TABLE chat_messages IS 'Messages of a chat session, as a conversation tree.';
COMMENT ON COLUMN chat_messages.id IS 'Unique message identifier';
COMMENT ON COLUMN chat_messages.session_id IS 'Session this message belongs to';
COMMENT ON COLUMN chat_messages.parent_id IS 'Parent in the conversation tree; NULL is a root';
COMMENT ON COLUMN chat_messages.role IS 'Author of the message (system, user, or assistant)';
COMMENT ON COLUMN chat_messages.content IS 'XChaCha20-Poly1305 encrypted message text';
COMMENT ON COLUMN chat_messages.created_at IS 'Message creation timestamp';
