-- Revert the assignments table.
-- Objects are dropped in reverse order of creation.

DROP TABLE IF EXISTS workspace_assignments;

DROP TYPE IF EXISTS ASSIGNMENT_STATUS;

-- The `file.assigned` / `file.unassigned` / `file.assignment.updated` labels added
-- to ACTIVITY_TYPE, WEBHOOK_EVENT, and NOTIFICATION_EVENT are intentionally left
-- in place: Postgres has no ALTER TYPE ... DROP VALUE, and the surviving labels
-- are inert once nothing references them.
