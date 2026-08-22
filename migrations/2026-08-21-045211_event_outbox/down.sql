-- Revert the event outbox table.
-- Objects are dropped in reverse order of creation.

DROP TABLE IF EXISTS event_outbox;

DROP TYPE IF EXISTS OUTBOX_STATUS;
