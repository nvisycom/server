//! Pipelines table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

use super::ConstraintCategory;

/// Pipelines table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum WorkspacePipelineConstraints {
    // Pipeline name validation constraints
    #[strum(serialize = "workspace_pipelines_name_length")]
    NameLength,

    // Pipeline description validation constraints
    #[strum(serialize = "workspace_pipelines_description_length")]
    DescriptionLength,

    // Pipeline definition constraints
    #[strum(serialize = "workspace_pipelines_definition_size")]
    DefinitionSize,

    // Pipeline metadata constraints
    #[strum(serialize = "workspace_pipelines_metadata_size")]
    MetadataSize,

    // Pipeline schedule validation constraints
    #[strum(serialize = "workspace_pipelines_schedule_cron_length")]
    ScheduleCronLength,
    #[strum(serialize = "workspace_pipelines_schedule_requires_cron")]
    ScheduleRequiresCron,
    #[strum(serialize = "workspace_pipelines_schedule_tz_length")]
    ScheduleTzLength,

    // Uniqueness constraints
    #[strum(serialize = "workspace_pipelines_workspace_id_id_key")]
    WorkspaceIdIdUnique,

    // Pipeline chronological constraints
    #[strum(serialize = "workspace_pipelines_updated_after_created")]
    UpdatedAfterCreated,
    #[strum(serialize = "workspace_pipelines_deleted_after_created")]
    DeletedAfterCreated,
}

impl WorkspacePipelineConstraints {
    /// Creates a new [`WorkspacePipelineConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }

    /// Returns the category of this constraint violation.
    pub fn categorize(&self) -> ConstraintCategory {
        match self {
            WorkspacePipelineConstraints::NameLength
            | WorkspacePipelineConstraints::DescriptionLength
            | WorkspacePipelineConstraints::DefinitionSize
            | WorkspacePipelineConstraints::MetadataSize
            | WorkspacePipelineConstraints::ScheduleCronLength
            | WorkspacePipelineConstraints::ScheduleRequiresCron
            | WorkspacePipelineConstraints::ScheduleTzLength => ConstraintCategory::Validation,

            WorkspacePipelineConstraints::WorkspaceIdIdUnique => ConstraintCategory::Uniqueness,

            WorkspacePipelineConstraints::UpdatedAfterCreated
            | WorkspacePipelineConstraints::DeletedAfterCreated => {
                ConstraintCategory::Chronological
            }
        }
    }
}

impl From<WorkspacePipelineConstraints> for String {
    #[inline]
    fn from(val: WorkspacePipelineConstraints) -> Self {
        val.to_string()
    }
}

impl TryFrom<String> for WorkspacePipelineConstraints {
    type Error = strum::ParseError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
