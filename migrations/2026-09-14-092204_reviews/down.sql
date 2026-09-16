-- Revert the document-review tables and their enum types.
-- Objects are dropped in reverse order of creation (children reference reviews).

DROP TABLE IF EXISTS workspace_review_events;
DROP TABLE IF EXISTS workspace_review_redactions;
DROP TABLE IF EXISTS workspace_review_detections;
DROP TABLE IF EXISTS workspace_review_assignees;
DROP TABLE IF EXISTS workspace_reviews;

DROP TYPE IF EXISTS REVIEW_EVENT_KIND;
DROP TYPE IF EXISTS REVIEW_STATUS;

-- The review.* labels added to ACTIVITY_TYPE, WEBHOOK_EVENT, and NOTIFICATION_EVENT
-- are intentionally left in place: Postgres has no ALTER TYPE ... DROP VALUE, and
-- the surviving labels are inert once nothing references them.
