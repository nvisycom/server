-- Revert the comments table.
-- Objects are dropped in reverse order of creation.

DROP TABLE IF EXISTS workspace_comments;

-- The comment.* labels added to ACTIVITY_TYPE, WEBHOOK_EVENT, and
-- NOTIFICATION_EVENT are intentionally left in place: Postgres has no
-- ALTER TYPE ... DROP VALUE, and the surviving labels are inert once nothing
-- references them.
