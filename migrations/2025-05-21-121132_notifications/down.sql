-- Revert the notifications table.
-- Objects are dropped in reverse order of creation.

DROP TABLE IF EXISTS account_notifications;

DROP TYPE IF EXISTS NOTIFICATION_EVENT;
