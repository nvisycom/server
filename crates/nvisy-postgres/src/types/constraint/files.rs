//! Files table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Files table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum WorkspaceFileConstraints {
    #[strum(serialize = "workspace_files_display_name_length")]
    DisplayNameLength,
    #[strum(serialize = "workspace_files_original_filename_length")]
    OriginalFilenameLength,
    #[strum(serialize = "workspace_files_file_extension_format")]
    FileExtensionFormat,
    #[strum(serialize = "workspace_files_file_size_min")]
    FileSizeMin,
    #[strum(serialize = "workspace_files_metadata_size")]
    MetadataSize,
    #[strum(serialize = "workspace_files_version_number_min")]
    VersionNumberMin,
    #[strum(serialize = "workspace_files_workspace_id_id_key")]
    WorkspaceIdIdUnique,
    #[strum(serialize = "workspace_files_source_object_unique_idx")]
    SourceObjectUnique,
}

impl WorkspaceFileConstraints {
    /// Creates a new [`WorkspaceFileConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }
}

impl From<WorkspaceFileConstraints> for String {
    #[inline]
    fn from(val: WorkspaceFileConstraints) -> Self {
        val.to_string()
    }
}

impl TryFrom<String> for WorkspaceFileConstraints {
    type Error = strum::ParseError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
