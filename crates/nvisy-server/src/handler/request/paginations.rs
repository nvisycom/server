//! Pagination request types for API endpoints.
//!
//! This module re-exports pagination types from nvisy-postgres and provides
//! API-specific wrappers with validation for HTTP query parameters.

use nvisy_postgres::types;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use validator::Validate;

/// Default pagination limit.
const DEFAULT_LIMIT: u32 = 20;
/// Maximum pagination limit.
const MAX_LIMIT: u32 = 100;
/// Maximum offset for offset-based pagination.
const MAX_OFFSET: u32 = 100_000;

/// Offset-based pagination query parameters.
///
/// Use this for admin dashboards or when users need to jump to specific pages.
/// For infinite scroll or API iteration, prefer [`CursorPagination`].
#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct OffsetPagination {
    /// The number of records to skip before starting to return results.
    #[validate(range(max = 100000))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<u32>,

    /// The maximum number of records to return (1-100, default: 20).
    #[validate(range(min = 1, max = 100))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

impl OffsetPagination {
    /// Returns the pagination offset.
    #[inline]
    pub fn offset(&self) -> u32 {
        self.offset.unwrap_or(0).min(MAX_OFFSET)
    }

    /// Returns the pagination limit.
    #[inline]
    pub fn limit(&self) -> u32 {
        self.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT)
    }
}

impl From<OffsetPagination> for types::OffsetPagination {
    fn from(query: OffsetPagination) -> Self {
        Self::new(query.limit() as i64, query.offset() as i64)
    }
}

/// Cursor-based pagination query parameters.
///
/// This is the preferred pagination method for API endpoints. It provides:
/// - Consistent performance regardless of page depth
/// - Stable results even when items are added/removed
/// - Efficient "load more" / infinite scroll patterns
#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct CursorPagination {
    /// The maximum number of records to return (1-100, default: 20).
    #[validate(range(min = 1, max = 100))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,

    /// Cursor pointing to the last item of the previous page.
    /// Obtain this from the `nextCursor` field in the response.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
}

impl CursorPagination {
    /// Returns the pagination limit.
    #[inline]
    pub fn limit(&self) -> u32 {
        self.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT)
    }
}

impl From<CursorPagination> for types::CursorPagination {
    fn from(query: CursorPagination) -> Self {
        Self::from_cursor_string(query.limit() as i64, query.after.as_deref())
    }
}
