//! Pagination types for database queries.
//!
//! Every list query pages by keyset cursor.

mod cursor;

pub use cursor::{Cursor, CursorKey, CursorPage, CursorPagination, TimeCursor};
pub(crate) use cursor::{keyset, total};
