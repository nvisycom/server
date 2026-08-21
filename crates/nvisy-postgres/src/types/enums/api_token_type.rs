//! API token type enumeration for authentication tracking.

use diesel_derive_enum::DbEnum;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Defines the type of API token for authentication and tracking purposes.
///
/// This enumeration corresponds to the `API_TOKEN_TYPE` PostgreSQL enum and is used
/// to categorize different types of authentication tokens based on the client type.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Serialize, Deserialize, DbEnum, Display, EnumIter, EnumString)]
#[ExistingTypePath = "crate::schema::sql_types::ApiTokenType"]
pub enum ApiTokenType {
    /// Web browser token (desktop or mobile browser)
    #[db_rename = "web"]
    #[serde(rename = "web")]
    #[default]
    Web,

    /// API client token (programmatic access)
    #[db_rename = "api"]
    #[serde(rename = "api")]
    Api,

    /// CLI tool token (command-line interface)
    #[db_rename = "cli"]
    #[serde(rename = "cli")]
    Cli,
}
