-- Revert the policies tables.
-- Objects are dropped in reverse order of creation.

-- workspace_pipeline_policies is the pipeline-to-policy join; drop it before
-- its parent policy table.
DROP TABLE IF EXISTS workspace_pipeline_policies;
DROP TABLE IF EXISTS workspace_policies;
