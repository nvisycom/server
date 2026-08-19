-- Revert the chat feature: drop messages, sessions, then the role enum.

DROP TABLE IF EXISTS chat_messages;
DROP TABLE IF EXISTS chat_sessions;
DROP TYPE IF EXISTS CHAT_ROLE;
