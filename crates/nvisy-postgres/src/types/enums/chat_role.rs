//! Chat message role enumeration.

use diesel_derive_enum::DbEnum;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// The author of a chat message.
///
/// Corresponds to the `CHAT_ROLE` PostgreSQL enum.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Serialize, Deserialize, DbEnum, Display, EnumIter, EnumString)]
#[ExistingTypePath = "crate::schema::sql_types::ChatRole"]
pub enum ChatRole {
    /// A system instruction (server-authored context).
    #[db_rename = "system"]
    #[serde(rename = "system")]
    System,

    /// A message from the account.
    #[db_rename = "user"]
    #[serde(rename = "user")]
    User,

    /// A reply from the model.
    #[db_rename = "assistant"]
    #[serde(rename = "assistant")]
    Assistant,
}
