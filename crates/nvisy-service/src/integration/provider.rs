//! Integration provider definitions and registry.

/// Functional category of an integration.
///
/// This mirrors `IntegrationType` from nvisy-postgres for use in the service layer
/// without requiring the postgres crate as a dependency.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegrationCategory {
    /// Files/documents (Drive, S3, SharePoint, Dropbox)
    Storage,
    /// Email, chat (Gmail, Slack, Teams)
    Communication,
    /// CRM, finance, legal (Salesforce, QuickBooks)
    Business,
    /// Data platforms (Snowflake, Tableau, Looker)
    Analytics,
    /// No-code automation (Zapier, Make)
    Automation,
    /// API/webhook integrations
    Developer,
    /// Specialized verticals (healthcare, insurance)
    Industry,
}

/// Direction of data flow for an integration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegrationDirection {
    /// Data flows into nvisy from the external service.
    Inbound,
    /// Data flows from nvisy to the external service.
    Outbound,
    /// Data flows in both directions.
    Bidirectional,
}

/// Authentication method used by an integration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegrationAuthType {
    /// OAuth 2.0 authentication flow.
    OAuth2,
    /// API key-based authentication.
    ApiKey,
    /// Webhook-based (no authentication on our side).
    Webhook,
}

/// OAuth 2.0 configuration for an integration.
#[derive(Debug, Clone)]
pub struct OAuthConfig {
    /// Authorization endpoint URL.
    pub authorize_url: &'static str,
    /// Token endpoint URL.
    pub token_url: &'static str,
    /// Required scopes for the integration.
    pub scopes: &'static [&'static str],
}

/// Metadata defining an integration provider.
#[derive(Debug, Clone)]
pub struct IntegrationProvider {
    /// Unique identifier (lowercase, e.g., "google_drive").
    pub name: &'static str,
    /// Functional category.
    pub category: IntegrationCategory,
    /// Direction of data flow.
    pub direction: IntegrationDirection,
    /// Authentication method.
    pub auth_type: IntegrationAuthType,
    /// Whether this is an external service (vs. internal webhook).
    pub is_external: bool,
    /// Whether this integration is currently enabled.
    pub is_enabled: bool,
    /// OAuth configuration, if applicable.
    pub oauth: Option<OAuthConfig>,
}
