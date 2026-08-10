//! Typed views of stored settings JSON: workspace settings and the data-
//! retention rules they carry.

mod retention;
mod workspace;

pub use retention::{
    PIPELINE_RETENTION_KEY, Retention, RetentionOverride, RetentionScope, RetentionSettings,
};
pub use workspace::WorkspaceSettings;
