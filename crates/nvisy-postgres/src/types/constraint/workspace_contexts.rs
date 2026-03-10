//! Workspace contexts table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

use super::ConstraintCategory;

/// Workspace contexts table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum WorkspaceContextConstraints {
    // Name validation constraints
    #[strum(serialize = "workspace_contexts_name_length")]
    NameLength,

    // Description validation constraints
    #[strum(serialize = "workspace_contexts_description_length")]
    DescriptionLength,

    // MIME type validation constraints
    #[strum(serialize = "workspace_contexts_mime_type_length")]
    MimeTypeLength,

    // Storage key validation constraints
    #[strum(serialize = "workspace_contexts_storage_key_length")]
    StorageKeyLength,

    // Content validation constraints
    #[strum(serialize = "workspace_contexts_content_size_positive")]
    ContentSizePositive,
    #[strum(serialize = "workspace_contexts_content_hash_length")]
    ContentHashLength,

    // Metadata validation constraints
    #[strum(serialize = "workspace_contexts_metadata_size")]
    MetadataSize,

    // Uniqueness constraints
    #[strum(serialize = "workspace_contexts_name_unique_idx")]
    NameUnique,

    // Chronological constraints
    #[strum(serialize = "workspace_contexts_updated_after_created")]
    UpdatedAfterCreated,
    #[strum(serialize = "workspace_contexts_deleted_after_created")]
    DeletedAfterCreated,
}

impl WorkspaceContextConstraints {
    /// Creates a new [`WorkspaceContextConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }

    /// Returns the category of this constraint violation.
    pub fn categorize(&self) -> ConstraintCategory {
        match self {
            WorkspaceContextConstraints::NameLength
            | WorkspaceContextConstraints::DescriptionLength
            | WorkspaceContextConstraints::MimeTypeLength
            | WorkspaceContextConstraints::StorageKeyLength
            | WorkspaceContextConstraints::ContentSizePositive
            | WorkspaceContextConstraints::ContentHashLength
            | WorkspaceContextConstraints::MetadataSize => ConstraintCategory::Validation,

            WorkspaceContextConstraints::NameUnique => ConstraintCategory::Uniqueness,

            WorkspaceContextConstraints::UpdatedAfterCreated
            | WorkspaceContextConstraints::DeletedAfterCreated => ConstraintCategory::Chronological,
        }
    }
}

impl From<WorkspaceContextConstraints> for String {
    #[inline]
    fn from(val: WorkspaceContextConstraints) -> Self {
        val.to_string()
    }
}

impl TryFrom<String> for WorkspaceContextConstraints {
    type Error = strum::ParseError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
