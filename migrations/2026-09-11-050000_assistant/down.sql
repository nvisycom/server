-- Revert the assistant outbox table and the reserved assistant account.
-- Objects are dropped in reverse order of creation.

DROP TABLE IF EXISTS workspace_assistant_jobs;

-- The account's comments (and any threads it authored) cascade away with it
-- (author_account_id ... ON DELETE CASCADE).
DELETE FROM accounts WHERE id = '00000000-0000-0000-0000-000000000a11';
