//! Pagination request types for API endpoints.
//!
//! This module re-exports pagination types from nvisy-postgres and provides
//! API-specific wrappers with validation for HTTP query parameters.

use garde::Validate;
use nvisy_postgres::types;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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
#[garde(allow_unvalidated)]
pub struct OffsetPagination {
    /// The number of records to skip before starting to return results.
    #[garde(range(max = 100000))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<u32>,

    /// The maximum number of records to return (1-100, default: 20).
    #[garde(range(min = 1, max = 100))]
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
#[garde(allow_unvalidated)]
pub struct CursorPagination {
    /// The maximum number of records to return (1-100, default: 20).
    #[garde(range(min = 1, max = 100))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,

    /// Cursor pointing to the last item of the previous page.
    /// Obtain this from the `nextCursor` field in the response.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,

    /// Whether to include the total item count in the response's `total` field.
    /// Defaults to `false`, since counting is an extra query; set it to `true`
    /// only when the count is actually needed.
    #[serde(default)]
    pub include_count: bool,
}

impl CursorPagination {
    /// Returns the pagination limit.
    #[inline]
    pub fn limit(&self) -> u32 {
        self.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT)
    }

    /// Converts to typed database pagination, decoding the opaque `after` cursor
    /// against the query's keyset `K`. An `after` that does not decode to `K` is
    /// treated as no cursor (a fresh first page). Defaults to descending; a caller
    /// that walks ascending sets it with
    /// [`with_direction`](types::CursorPagination::with_direction).
    pub fn into_cursor<K>(self) -> types::CursorPagination<K>
    where
        K: types::CursorKey,
    {
        let pagination =
            types::CursorPagination::from_cursor_string(self.limit() as i64, self.after.as_deref());
        if self.include_count {
            pagination.with_count()
        } else {
            pagination
        }
    }
}
