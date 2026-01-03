//! Workspace integrations table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

use super::ConstraintCategory;

/// Workspace integrations table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum WorkspaceIntegrationConstraints {
    // Integration validation constraints
    #[strum(serialize = "workspace_integrations_integration_name_not_empty")]
    IntegrationNameNotEmpty,
    #[strum(serialize = "workspace_integrations_description_length_max")]
    DescriptionLengthMax,
    #[strum(serialize = "workspace_integrations_metadata_size")]
    MetadataSize,
    #[strum(serialize = "workspace_integrations_credentials_size")]
    CredentialsSize,

    // Integration chronological constraints
    #[strum(serialize = "workspace_integrations_updated_after_created")]
    UpdatedAfterCreated,
    #[strum(serialize = "workspace_integrations_last_sync_after_created")]
    LastSyncAfterCreated,
}

impl WorkspaceIntegrationConstraints {
    /// Creates a new [`WorkspaceIntegrationConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }

    /// Returns the category of this constraint violation.
    pub fn categorize(&self) -> ConstraintCategory {
        match self {
            WorkspaceIntegrationConstraints::IntegrationNameNotEmpty
            | WorkspaceIntegrationConstraints::DescriptionLengthMax
            | WorkspaceIntegrationConstraints::MetadataSize
            | WorkspaceIntegrationConstraints::CredentialsSize => ConstraintCategory::Validation,

            WorkspaceIntegrationConstraints::UpdatedAfterCreated
            | WorkspaceIntegrationConstraints::LastSyncAfterCreated => {
                ConstraintCategory::Chronological
            }
        }
    }
}

impl From<WorkspaceIntegrationConstraints> for String {
    #[inline]
    fn from(val: WorkspaceIntegrationConstraints) -> Self {
        val.to_string()
    }
}

impl TryFrom<String> for WorkspaceIntegrationConstraints {
    type Error = strum::ParseError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
