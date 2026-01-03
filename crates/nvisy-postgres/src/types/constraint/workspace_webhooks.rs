//! Workspace webhooks table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

use super::ConstraintCategory;

/// Workspace webhooks table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum WorkspaceWebhookConstraints {
    // Webhook validation constraints
    #[strum(serialize = "workspace_webhooks_display_name_length")]
    DisplayNameLength,
    #[strum(serialize = "workspace_webhooks_description_length")]
    DescriptionLength,
    #[strum(serialize = "workspace_webhooks_url_length")]
    UrlLength,
    #[strum(serialize = "workspace_webhooks_url_format")]
    UrlFormat,
    #[strum(serialize = "workspace_webhooks_secret_length")]
    SecretLength,
    #[strum(serialize = "workspace_webhooks_events_not_empty")]
    EventsNotEmpty,
    #[strum(serialize = "workspace_webhooks_headers_size")]
    HeadersSize,
    #[strum(serialize = "workspace_webhooks_failure_count_positive")]
    FailureCountPositive,
    #[strum(serialize = "workspace_webhooks_max_failures_positive")]
    MaxFailuresPositive,

    // Webhook chronological constraints
    #[strum(serialize = "workspace_webhooks_updated_after_created")]
    UpdatedAfterCreated,
    #[strum(serialize = "workspace_webhooks_deleted_after_created")]
    DeletedAfterCreated,
}

impl WorkspaceWebhookConstraints {
    /// Creates a new [`WorkspaceWebhookConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }

    /// Returns the category of this constraint violation.
    pub fn categorize(&self) -> ConstraintCategory {
        match self {
            WorkspaceWebhookConstraints::DisplayNameLength
            | WorkspaceWebhookConstraints::DescriptionLength
            | WorkspaceWebhookConstraints::UrlLength
            | WorkspaceWebhookConstraints::UrlFormat
            | WorkspaceWebhookConstraints::SecretLength
            | WorkspaceWebhookConstraints::EventsNotEmpty
            | WorkspaceWebhookConstraints::HeadersSize
            | WorkspaceWebhookConstraints::FailureCountPositive
            | WorkspaceWebhookConstraints::MaxFailuresPositive => ConstraintCategory::Validation,

            WorkspaceWebhookConstraints::UpdatedAfterCreated
            | WorkspaceWebhookConstraints::DeletedAfterCreated => ConstraintCategory::Chronological,
        }
    }
}

impl From<WorkspaceWebhookConstraints> for String {
    #[inline]
    fn from(val: WorkspaceWebhookConstraints) -> Self {
        val.to_string()
    }
}

impl TryFrom<String> for WorkspaceWebhookConstraints {
    type Error = strum::ParseError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
