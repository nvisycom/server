//! Project integrations table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

use super::ConstraintCategory;

/// Project integrations table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum ProjectIntegrationConstraints {
    // Integration validation constraints
    #[strum(serialize = "project_integrations_integration_name_not_empty")]
    IntegrationNameNotEmpty,
    #[strum(serialize = "project_integrations_description_length_max")]
    DescriptionLengthMax,
    #[strum(serialize = "project_integrations_metadata_size")]
    MetadataSize,
    #[strum(serialize = "project_integrations_credentials_size")]
    CredentialsSize,

    // Integration chronological constraints
    #[strum(serialize = "project_integrations_updated_after_created")]
    UpdatedAfterCreated,
    #[strum(serialize = "project_integrations_last_sync_after_created")]
    LastSyncAfterCreated,
}

impl ProjectIntegrationConstraints {
    /// Creates a new [`ProjectIntegrationConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }

    /// Returns the category of this constraint violation.
    pub fn categorize(&self) -> ConstraintCategory {
        match self {
            ProjectIntegrationConstraints::IntegrationNameNotEmpty
            | ProjectIntegrationConstraints::DescriptionLengthMax
            | ProjectIntegrationConstraints::MetadataSize
            | ProjectIntegrationConstraints::CredentialsSize => ConstraintCategory::Validation,

            ProjectIntegrationConstraints::UpdatedAfterCreated
            | ProjectIntegrationConstraints::LastSyncAfterCreated => {
                ConstraintCategory::Chronological
            }
        }
    }
}

impl From<ProjectIntegrationConstraints> for String {
    #[inline]
    fn from(val: ProjectIntegrationConstraints) -> Self {
        val.to_string()
    }
}

impl TryFrom<String> for ProjectIntegrationConstraints {
    type Error = strum::ParseError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
