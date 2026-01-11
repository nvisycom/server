-- Revert studio migration

-- Drop tables in reverse order (respecting foreign key dependencies)
DROP TABLE IF EXISTS studio_operations;
DROP TABLE IF EXISTS studio_tool_calls;
DROP TABLE IF EXISTS studio_sessions;

-- Drop enums
DROP TYPE IF EXISTS STUDIO_TOOL_STATUS;
DROP TYPE IF EXISTS STUDIO_SESSION_STATUS;
