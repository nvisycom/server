-- Assignments: distribute redaction-review work over a file to workspace
-- members. A file can be assigned to several reviewers at once (like GitHub
-- assignees), each with their own review status, so an assignment is a
-- first-class row keyed per (file, reviewer) rather than a column on the file.

-- The review status of one reviewer's assignment on a file. This is the human
-- review-workflow axis, independent of a detection's execution status
-- (DETECTION_STATUS), which is driven by the analysis worker.
CREATE TYPE ASSIGNMENT_STATUS AS ENUM (
    'assigned',     -- Assigned to the reviewer; not yet started
    'in_review',    -- The reviewer has started reviewing
    'done'          -- The reviewer has finished their review
);

COMMENT ON TYPE ASSIGNMENT_STATUS IS 'Review-workflow status of one reviewer''s assignment on a file: assigned, in_review, or done.';

-- Assignments table: one reviewer's assignment of one file.
CREATE TABLE workspace_assignments (
    -- Primary identifier
    id                     UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References. The workspace is denormalized onto the row (rather than reached
    -- through the file) so the common "my assignments across the workspace" query
    -- is a single indexed scan with no join to workspace_files.
    workspace_id           UUID              NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    file_id                UUID              NOT NULL,

    -- The two accounts an assignment relates to, each a distinct role.
    --   assignee: the reviewer. If their account is removed, the assignment goes
    --             with it (CASCADE).
    --   assigned: who created the assignment, kept for the audit trail. SET NULL
    --             rather than CASCADE so the assigner leaving does not delete a
    --             live assignment; null then means "assigner gone".
    assignee_account_id    UUID              NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    assigned_account_id    UUID              DEFAULT NULL REFERENCES accounts (id) ON DELETE SET NULL,

    -- The reviewer's current review status for this file.
    status                 ASSIGNMENT_STATUS NOT NULL DEFAULT 'assigned',

    -- Timestamps
    created_at             TIMESTAMPTZ       NOT NULL DEFAULT current_timestamp,
    updated_at             TIMESTAMPTZ       NOT NULL DEFAULT current_timestamp,
    CONSTRAINT workspace_assignments_updated_after_created CHECK (updated_at >= created_at),

    -- The file is referenced with its workspace, against
    -- workspace_files (workspace_id, id), so the denormalized workspace_id must
    -- match the file's own — a file from another workspace cannot be stored. A
    -- whole-workspace teardown, or removing the file, cascades its assignments
    -- away together.
    CONSTRAINT workspace_assignments_file_fkey FOREIGN KEY (workspace_id, file_id)
        REFERENCES workspace_files (workspace_id, id) ON DELETE CASCADE,

    -- A reviewer is assigned a given file at most once; assigning again is a
    -- conflict, and reassignment is remove-then-add, not a second row.
    CONSTRAINT workspace_assignments_file_assignee_key UNIQUE (file_id, assignee_account_id)
);

-- A reviewer's assignments across the workspace, newest first ("my work").
CREATE INDEX workspace_assignments_assignee_idx
    ON workspace_assignments (assignee_account_id, created_at DESC);

-- A file's reviewer list ("who is assigned to this file").
CREATE INDEX workspace_assignments_file_idx
    ON workspace_assignments (file_id, created_at DESC);

-- Workspace-scoped listing/board, filterable by status, newest first.
CREATE INDEX workspace_assignments_workspace_idx
    ON workspace_assignments (workspace_id, created_at DESC);

-- Auto-maintain updated_at on writes (no soft-delete column here).
SELECT setup_updated_at_no_soft_delete('workspace_assignments');

COMMENT ON TABLE workspace_assignments IS 'Per-reviewer assignment of a file for redaction review; a file may have several.';
COMMENT ON COLUMN workspace_assignments.id IS 'Unique assignment identifier';
COMMENT ON COLUMN workspace_assignments.workspace_id IS 'Denormalized workspace scope for fast per-workspace assignment queries';
COMMENT ON COLUMN workspace_assignments.file_id IS 'File under review';
COMMENT ON COLUMN workspace_assignments.assignee_account_id IS 'Reviewer the file is assigned to';
COMMENT ON COLUMN workspace_assignments.assigned_account_id IS 'Account that created the assignment, for the audit trail; null if that account was removed';
COMMENT ON COLUMN workspace_assignments.status IS 'The reviewer''s review status for this file';
COMMENT ON COLUMN workspace_assignments.created_at IS 'When the assignment was created';
COMMENT ON COLUMN workspace_assignments.updated_at IS 'When the assignment was last updated';

-- Assignment lifecycle events feed the three event sinks, so their type strings
-- are added to each sink's enum. Postgres 12+ permits ALTER TYPE ... ADD VALUE
-- inside a transaction as long as the value is not used in the same one; this
-- migration only adds the labels (no rows use them yet), so it stays
-- transactional like the others.
--
-- Activity log records all three (assigned, unassigned, status changed).
ALTER TYPE ACTIVITY_TYPE ADD VALUE IF NOT EXISTS 'file.assigned';
ALTER TYPE ACTIVITY_TYPE ADD VALUE IF NOT EXISTS 'file.unassigned';
ALTER TYPE ACTIVITY_TYPE ADD VALUE IF NOT EXISTS 'file.assignment.updated';

-- Webhooks carry all three.
ALTER TYPE WEBHOOK_EVENT ADD VALUE IF NOT EXISTS 'file.assigned';
ALTER TYPE WEBHOOK_EVENT ADD VALUE IF NOT EXISTS 'file.unassigned';
ALTER TYPE WEBHOOK_EVENT ADD VALUE IF NOT EXISTS 'file.assignment.updated';

-- In-app notifications go to the reviewer on assign and unassign; a status
-- change raises no notification, so it is not added here.
ALTER TYPE NOTIFICATION_EVENT ADD VALUE IF NOT EXISTS 'file.assigned';
ALTER TYPE NOTIFICATION_EVENT ADD VALUE IF NOT EXISTS 'file.unassigned';
