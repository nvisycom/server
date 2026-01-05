//! Integration type enumeration for categorizing workspace integrations.

use diesel_derive_enum::DbEnum;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Defines the functional category of a workspace integration.
///
/// This enumeration corresponds to the `INTEGRATION_TYPE` PostgreSQL enum and is used
/// to categorize different types of third-party integrations that can be connected to workspaces.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[derive(Serialize, Deserialize, DbEnum, Display, EnumIter, EnumString)]
#[ExistingTypePath = "crate::schema::sql_types::IntegrationType"]
pub enum IntegrationType {
    /// Files/documents (Drive, S3, SharePoint, Dropbox)
    #[db_rename = "storage"]
    #[serde(rename = "storage")]
    #[default]
    Storage,

    /// Email, chat (Gmail, Slack, Teams)
    #[db_rename = "communication"]
    #[serde(rename = "communication")]
    Communication,

    /// CRM, finance, legal (Salesforce, QuickBooks)
    #[db_rename = "business"]
    #[serde(rename = "business")]
    Business,

    /// Data platforms (Snowflake, Tableau, Looker)
    #[db_rename = "analytics"]
    #[serde(rename = "analytics")]
    Analytics,

    /// No-code automation (Zapier, Make)
    #[db_rename = "automation"]
    #[serde(rename = "automation")]
    Automation,

    /// API/webhook integrations
    #[db_rename = "developer"]
    #[serde(rename = "developer")]
    Developer,

    /// Specialized verticals (healthcare, insurance)
    #[db_rename = "industry"]
    #[serde(rename = "industry")]
    Industry,
}
