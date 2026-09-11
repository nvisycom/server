//! Pagination types for database queries.
//!
//! This module provides both offset-based and cursor-based pagination,
//! with cursor-based being the preferred approach for most use cases.

mod cursor;
mod offset;

pub(crate) use cursor::keyset;
pub use cursor::{Cursor, CursorKey, CursorPage, CursorPagination};
pub use offset::{OffsetPage, OffsetPagination};
