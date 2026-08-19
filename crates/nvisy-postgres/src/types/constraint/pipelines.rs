//! Pipelines table constraint violations.

use strum::EnumString;

/// Pipelines table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq, EnumString)]
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
