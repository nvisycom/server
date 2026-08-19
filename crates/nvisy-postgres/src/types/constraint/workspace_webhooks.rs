//! Workspace webhooks table constraint violations.

use strum::EnumString;

/// Workspace webhooks table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq, EnumString)]
pub enum WorkspaceWebhookConstraints {
    #[strum(serialize = "workspace_webhooks_workspace_id_id_key")]
    WorkspaceIdIdUnique,
    #[strum(serialize = "workspace_webhooks_display_name_length")]
    DisplayNameLength,
    #[strum(serialize = "workspace_webhooks_description_length")]
    DescriptionLength,
    #[strum(serialize = "workspace_webhooks_url_length")]
    UrlLength,
    #[strum(serialize = "workspace_webhooks_url_format")]
    UrlFormat,
    #[strum(serialize = "workspace_webhooks_events_not_empty")]
    EventsNotEmpty,
    #[strum(serialize = "workspace_webhooks_headers_size")]
    HeadersSize,
}
