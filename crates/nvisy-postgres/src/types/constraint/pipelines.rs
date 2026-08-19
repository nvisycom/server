//! Pipelines table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Pipelines table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum WorkspacePipelineConstraints {
    #[strum(serialize = "workspace_pipelines_slug_length")]
    SlugLength,
    #[strum(serialize = "workspace_pipelines_slug_format")]
    SlugFormat,
    #[strum(serialize = "workspace_pipelines_display_name_length")]
    NameLength,
    #[strum(serialize = "workspace_pipelines_description_length")]
    DescriptionLength,
    #[strum(serialize = "workspace_pipelines_definition_size")]
    DefinitionSize,
    #[strum(serialize = "workspace_pipelines_metadata_size")]
    MetadataSize,
    #[strum(serialize = "workspace_pipelines_schedule_cron_length")]
    ScheduleCronLength,
    #[strum(serialize = "workspace_pipelines_schedule_requires_cron")]
    ScheduleRequiresCron,
    #[strum(serialize = "workspace_pipelines_schedule_tz_length")]
    ScheduleTzLength,
    #[strum(serialize = "workspace_pipelines_workspace_id_id_key")]
    WorkspaceIdIdUnique,
    #[strum(serialize = "workspace_pipelines_slug_unique_idx")]
    SlugUnique,
}

impl WorkspacePipelineConstraints {
    /// Creates a new [`WorkspacePipelineConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
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
