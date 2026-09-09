//! Pipeline trigger type enumeration indicating how a pipeline run was initiated.

use super::db_enum;

db_enum! {
    /// How a pipeline run was initiated.
    ///
    /// Corresponds to the `PIPELINE_TRIGGER_TYPE` PostgreSQL enum: a run is either
    /// started directly by a user or automatically by the system (for example, a
    /// file upload that the pipeline auto-redacts).
    pub enum PipelineTriggerType: Default = User, "crate::schema::sql_types::PipelineTriggerType" {
        /// Started directly by a user.
        User = "user",
        /// Started automatically by the system (e.g. a file upload auto-redacted
        /// by the pipeline).
        System = "system",
    }
}
