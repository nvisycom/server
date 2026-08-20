-- Revert the chat feature.
-- Objects are dropped in reverse order of creation.

-- chat_messages and chat_sessions reference each other; dropping both in one
-- statement resolves the cross constraint without CASCADE (which could silently
-- remove unexpected external dependents).
DROP TABLE IF EXISTS chat_messages, chat_sessions;

DROP TYPE IF EXISTS CHAT_ROLE;
