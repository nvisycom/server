//! Chat sessions table constraint violations.

use strum::EnumString;

/// Chat sessions table constraint violations.
///
/// Enumerates the constraints a client request can trip that map to a specific
/// non-500 response. Server-controlled invariants (ownership and active-leaf
/// foreign keys, timestamp ordering) fall through to the generic handler.
#[derive(Debug, Clone, Copy, Eq, PartialEq, EnumString)]
pub enum ChatSessionConstraints {
    #[strum(serialize = "chat_sessions_title_length")]
    TitleLength,
}
