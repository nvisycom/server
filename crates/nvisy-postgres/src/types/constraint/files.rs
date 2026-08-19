//! Files table constraint violations.

use strum::EnumString;

/// Files table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq, EnumString)]
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
