-- Revert notifications

DROP FUNCTION IF EXISTS cleanup_expired_notifications();
DROP TABLE IF EXISTS account_notifications;
DROP TYPE IF EXISTS NOTIFICATION_EVENT;
