-- Revert the chat feature.
-- Objects are dropped in reverse order of creation.

-- chat_messages and chat_sessions reference each other; CASCADE clears the
-- cross constraint so both tables can be dropped.
DROP TABLE IF EXISTS chat_messages CASCADE;
DROP TABLE IF EXISTS chat_sessions;

DROP TYPE IF EXISTS CHAT_ROLE;
