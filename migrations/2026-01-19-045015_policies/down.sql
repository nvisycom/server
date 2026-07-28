-- Revert policies

-- Pipeline → policy join table (drop before its parent policy table).
DROP TABLE IF EXISTS workspace_pipeline_policies;

DROP TABLE IF EXISTS workspace_policies;
