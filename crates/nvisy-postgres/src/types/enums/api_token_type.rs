//! API token type enumeration for authentication tracking.

use diesel_derive_enum::DbEnum;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};
#[cfg(feature = "utoipa")]
use utoipa::ToSchema;

/// Defines the type of API token for authentication and tracking purposes.
///
/// This enumeration corresponds to the `API_TOKEN_TYPE` PostgreSQL enum and is used
/// to categorize different types of authentication tokens based on the client type.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, DbEnum, Display, EnumIter, EnumString)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[ExistingTypePath = "crate::schema::sql_types::ApiTokenType"]
pub enum ApiTokenType {
    /// Web browser token (desktop or mobile browser)
    #[db_rename = "web"]
    #[serde(rename = "web")]
    #[default]
    Web,

    /// Mobile application token (iOS/Android native apps)
    #[db_rename = "mobile"]
    #[serde(rename = "mobile")]
    Mobile,

    /// API client token (programmatic access)
    #[db_rename = "api"]
    #[serde(rename = "api")]
    Api,

    /// Desktop application token (native desktop apps)
    #[db_rename = "desktop"]
    #[serde(rename = "desktop")]
    Desktop,
}

impl ApiTokenType {
    /// Returns whether this token type typically supports interactive user interfaces.
    #[inline]
    pub fn is_interactive(self) -> bool {
        matches!(
            self,
            ApiTokenType::Web | ApiTokenType::Mobile | ApiTokenType::Desktop
        )
    }

    /// Returns whether this token type is programmatic (non-interactive).
    #[inline]
    pub fn is_programmatic(self) -> bool {
        matches!(self, ApiTokenType::Api)
    }
}
