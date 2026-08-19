-- Revert the webhooks table.
-- Objects are dropped in reverse order of creation.

DROP TABLE IF EXISTS workspace_webhooks;

DROP TYPE IF EXISTS WEBHOOK_EVENT;
DROP TYPE IF EXISTS WEBHOOK_STATUS;
