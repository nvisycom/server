//! Project webhooks table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

use super::ConstraintCategory;

/// Project webhooks table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum ProjectWebhookConstraints {
    // Webhook validation constraints
    #[strum(serialize = "project_webhooks_display_name_length")]
    DisplayNameLength,
    #[strum(serialize = "project_webhooks_description_length")]
    DescriptionLength,
    #[strum(serialize = "project_webhooks_url_length")]
    UrlLength,
    #[strum(serialize = "project_webhooks_url_format")]
    UrlFormat,
    #[strum(serialize = "project_webhooks_secret_length")]
    SecretLength,
    #[strum(serialize = "project_webhooks_events_not_empty")]
    EventsNotEmpty,
    #[strum(serialize = "project_webhooks_headers_size")]
    HeadersSize,
    #[strum(serialize = "project_webhooks_failure_count_positive")]
    FailureCountPositive,
    #[strum(serialize = "project_webhooks_max_failures_positive")]
    MaxFailuresPositive,

    // Webhook chronological constraints
    #[strum(serialize = "project_webhooks_updated_after_created")]
    UpdatedAfterCreated,
    #[strum(serialize = "project_webhooks_deleted_after_created")]
    DeletedAfterCreated,
}

impl ProjectWebhookConstraints {
    /// Creates a new [`ProjectWebhookConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }

    /// Returns the category of this constraint violation.
    pub fn categorize(&self) -> ConstraintCategory {
        match self {
            ProjectWebhookConstraints::DisplayNameLength
            | ProjectWebhookConstraints::DescriptionLength
            | ProjectWebhookConstraints::UrlLength
            | ProjectWebhookConstraints::UrlFormat
            | ProjectWebhookConstraints::SecretLength
            | ProjectWebhookConstraints::EventsNotEmpty
            | ProjectWebhookConstraints::HeadersSize
            | ProjectWebhookConstraints::FailureCountPositive
            | ProjectWebhookConstraints::MaxFailuresPositive => ConstraintCategory::Validation,

            ProjectWebhookConstraints::UpdatedAfterCreated
            | ProjectWebhookConstraints::DeletedAfterCreated => ConstraintCategory::Chronological,
        }
    }
}

impl From<ProjectWebhookConstraints> for String {
    #[inline]
    fn from(val: ProjectWebhookConstraints) -> Self {
        val.to_string()
    }
}

impl TryFrom<String> for ProjectWebhookConstraints {
    type Error = strum::ParseError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
