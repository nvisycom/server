//! Documents table constraint violations.

use strum::EnumString;

/// Documents table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq, EnumString)]
pub enum WorkspaceDocumentConstraints {
    #[strum(serialize = "workspace_documents_display_name_length")]
    DisplayNameLength,
    #[strum(serialize = "workspace_documents_original_filename_length")]
    OriginalFilenameLength,
    #[strum(serialize = "workspace_documents_file_extension_format")]
    FileExtensionFormat,
    #[strum(serialize = "workspace_documents_metadata_size")]
    MetadataSize,
    #[strum(serialize = "workspace_documents_workspace_id_id_key")]
    WorkspaceIdIdUnique,
}
