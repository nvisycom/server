//! Join model linking a pipeline to the policies it references.
//!
//! References are relational (real foreign keys) rather than embedded in the
//! pipeline's JSON definition, so the database enforces integrity and cleans up
//! on cascade. The composite key pins every reference to a single workspace.

use diesel::prelude::*;
use uuid::Uuid;

use crate::schema::workspace_pipeline_policies;

/// A pipeline → policy reference row.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable, Insertable)]
#[diesel(table_name = workspace_pipeline_policies)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct PipelinePolicy {
    /// Workspace both the pipeline and policy belong to.
    pub workspace_id: Uuid,
    /// Referencing pipeline.
    pub pipeline_id: Uuid,
    /// Referenced policy.
    pub policy_id: Uuid,
}
