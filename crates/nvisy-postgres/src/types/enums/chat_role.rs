//! Chat message role enumeration.

use super::db_enum;

db_enum! {
    /// The author of a chat message.
    ///
    /// Corresponds to the `CHAT_ROLE` PostgreSQL enum.
    pub enum ChatRole = "crate::schema::sql_types::ChatRole" {
        /// A system instruction (server-authored context).
        System = "system",
        /// A message from the account.
        User = "user",
        /// A reply from the model.
        Assistant = "assistant",
    }
}
