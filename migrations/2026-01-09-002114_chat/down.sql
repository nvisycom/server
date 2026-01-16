-- Revert chat migration

-- Drop tables in reverse order (respecting foreign key dependencies)
DROP TABLE IF EXISTS chat_operations;
DROP TABLE IF EXISTS chat_tool_calls;
DROP TABLE IF EXISTS chat_sessions;

-- Drop enums
DROP TYPE IF EXISTS CHAT_TOOL_STATUS;
DROP TYPE IF EXISTS CHAT_SESSION_STATUS;
