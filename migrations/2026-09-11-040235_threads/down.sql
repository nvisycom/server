-- Revert the thread tables and the thread-event-kind type.
-- Objects are dropped in reverse order of creation (children reference threads).

DROP TABLE IF EXISTS workspace_thread_events;
DROP TABLE IF EXISTS workspace_thread_comments;
DROP TABLE IF EXISTS workspace_threads;

DROP TYPE IF EXISTS THREAD_EVENT_KIND;
DROP TYPE IF EXISTS REVIEW_STATUS;

-- The thread.* / comment.* labels added to ACTIVITY_TYPE, WEBHOOK_EVENT, and
-- NOTIFICATION_EVENT are intentionally left in place: Postgres has no
-- ALTER TYPE ... DROP VALUE, and the surviving labels are inert once nothing
-- references them.
