//! Pipeline status enumeration indicating the lifecycle state of a pipeline.

use super::db_enum;

db_enum! {
    /// The lifecycle status of a pipeline definition.
    ///
    /// Corresponds to the `PIPELINE_STATUS` PostgreSQL enum and tracks whether a
    /// pipeline is being configured, enabled and ready to run, or disabled.
    pub enum PipelineStatus: Default = Draft, "crate::schema::sql_types::PipelineStatus" {
        /// Pipeline is being configured.
        Draft = "draft",
        /// Pipeline is ready to run.
        Enabled = "enabled",
        /// Pipeline is disabled.
        Disabled = "disabled",
    }
}

impl PipelineStatus {
    /// Returns whether the pipeline is enabled.
    #[inline]
    pub fn is_enabled(self) -> bool {
        matches!(self, PipelineStatus::Enabled)
    }
}
