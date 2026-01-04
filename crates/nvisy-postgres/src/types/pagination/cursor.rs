//! Cursor-based pagination for database queries.
//!
//! Cursor pagination provides efficient, stable pagination for large datasets.
//! Unlike offset pagination, performance remains constant regardless of page depth.

use base64::prelude::*;
use jiff::Timestamp;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Maximum number of items per page.
pub const MAX_LIMIT: i64 = 100;

/// A cursor representing a position in a paginated result set.
///
/// The cursor encodes the last seen item's timestamp and ID, allowing
/// efficient keyset pagination. The ID serves as a tiebreaker for items
/// with identical timestamps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(into = "String", try_from = "String")]
pub struct Cursor {
    /// Timestamp of the last seen item.
    pub timestamp: Timestamp,
    /// ID of the last seen item (tiebreaker).
    pub id: Uuid,
}

impl Cursor {
    /// Creates a new cursor from a timestamp and ID.
    pub fn new(timestamp: Timestamp, id: Uuid) -> Self {
        Self { timestamp, id }
    }

    /// Encodes the cursor as a URL-safe base64 string.
    pub fn encode(&self) -> String {
        let data = format!("{}|{}", self.timestamp, self.id);
        BASE64_URL_SAFE_NO_PAD.encode(data.as_bytes())
    }

    /// Decodes a cursor from a URL-safe base64 string.
    ///
    /// Returns `None` if the string is invalid or malformed.
    pub fn decode(encoded: &str) -> Option<Self> {
        let bytes = BASE64_URL_SAFE_NO_PAD.decode(encoded).ok()?;
        let data = String::from_utf8(bytes).ok()?;
        let (timestamp_str, id_str) = data.split_once('|')?;

        let timestamp = timestamp_str.parse().ok()?;
        let id = id_str.parse().ok()?;

        Some(Self { timestamp, id })
    }
}

impl std::fmt::Display for Cursor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.encode())
    }
}

impl From<Cursor> for String {
    fn from(cursor: Cursor) -> Self {
        cursor.encode()
    }
}

impl TryFrom<String> for Cursor {
    type Error = &'static str;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Cursor::decode(&value).ok_or("invalid cursor format")
    }
}

/// Cursor-based pagination parameters for database queries.
///
/// This is the preferred pagination method for API endpoints. It provides:
/// - Consistent performance regardless of page depth
/// - Stable results even when items are added/removed
/// - Efficient "load more" / infinite scroll patterns
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct CursorPagination {
    /// Maximum number of records to return.
    pub limit: i64,
    /// Cursor pointing to the last item of the previous page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after: Option<Cursor>,
    /// Whether to include total count in the response.
    /// Set to `false` to skip the count query for better performance.
    #[serde(default)]
    pub include_count: bool,
}

impl CursorPagination {
    /// Creates a new cursor pagination with the given limit.
    pub fn new(limit: i64) -> Self {
        Self {
            limit: limit.clamp(1, MAX_LIMIT),
            after: None,
            include_count: false,
        }
    }

    /// Creates cursor pagination starting after the given cursor.
    pub fn after(limit: i64, cursor: Cursor) -> Self {
        Self {
            limit: limit.clamp(1, MAX_LIMIT),
            after: Some(cursor),
            include_count: false,
        }
    }

    /// Creates cursor pagination from an optional encoded cursor string.
    ///
    /// If the cursor string is invalid, pagination starts from the beginning.
    pub fn from_cursor_string(limit: i64, cursor: Option<&str>) -> Self {
        Self {
            limit: limit.clamp(1, MAX_LIMIT),
            after: cursor.and_then(Cursor::decode),
            include_count: false,
        }
    }

    /// Enables including total count in the response.
    pub fn with_count(mut self) -> Self {
        self.include_count = true;
        self
    }

    /// Returns the limit plus one for fetching to determine if there are more results.
    ///
    /// When querying, fetch `limit + 1` items. If you get `limit + 1` results,
    /// there are more pages; return only `limit` items to the client.
    pub fn fetch_limit(&self) -> i64 {
        self.limit + 1
    }

    /// Checks if we have a cursor to paginate from.
    pub fn has_cursor(&self) -> bool {
        self.after.is_some()
    }
}

/// Result of a cursor-paginated query.
#[derive(Debug, Clone)]
pub struct CursorPage<T> {
    /// The items in this page.
    pub items: Vec<T>,
    /// Total count of items matching the query (across all pages).
    /// Only present if `include_count` was set in the pagination request.
    pub total: Option<i64>,
    /// Cursor to fetch the next page. Present only when more items exist.
    pub next_cursor: Option<String>,
}

impl<T> CursorPage<T> {
    /// Creates a new cursor page from query results.
    ///
    /// # Arguments
    /// * `items` - Items fetched from the database (should be `limit + 1` if there are more)
    /// * `total` - Total count of items matching the query (None if count was skipped)
    /// * `limit` - The requested page size
    /// * `cursor_fn` - Function to extract cursor data (timestamp, id) from an item
    pub fn new<F>(mut items: Vec<T>, total: Option<i64>, limit: i64, cursor_fn: F) -> Self
    where
        F: Fn(&T) -> (Timestamp, Uuid),
    {
        let has_more = items.len() as i64 > limit;

        // Remove the extra item used to detect more pages
        if has_more {
            items.pop();
        }

        let next_cursor = if has_more {
            items.last().map(|item| {
                let (timestamp, id) = cursor_fn(item);
                Cursor::new(timestamp, id).encode()
            })
        } else {
            None
        };

        Self {
            items,
            total,
            next_cursor,
        }
    }

    /// Creates an empty cursor page.
    pub fn empty() -> Self {
        Self {
            items: Vec::new(),
            total: Some(0),
            next_cursor: None,
        }
    }

    /// Returns true if there are more items to fetch.
    pub fn has_more(&self) -> bool {
        self.next_cursor.is_some()
    }

    /// Maps the items to a different type.
    pub fn map<U, F>(self, f: F) -> CursorPage<U>
    where
        F: FnMut(T) -> U,
    {
        CursorPage {
            items: self.items.into_iter().map(f).collect(),
            total: self.total,
            next_cursor: self.next_cursor,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_encode_decode_roundtrip() {
        let timestamp = Timestamp::now();
        let id = Uuid::new_v4();
        let cursor = Cursor::new(timestamp, id);

        let encoded = cursor.encode();
        let decoded = Cursor::decode(&encoded).expect("decode should succeed");

        assert_eq!(cursor.timestamp, decoded.timestamp);
        assert_eq!(cursor.id, decoded.id);
    }

    #[test]
    fn cursor_decode_invalid() {
        assert!(Cursor::decode("invalid").is_none());
        assert!(Cursor::decode("").is_none());
        assert!(Cursor::decode("not:valid:cursor").is_none());
    }

    #[test]
    fn cursor_pagination_defaults() {
        let pagination = CursorPagination::default();
        assert_eq!(pagination.limit, 0);
        assert!(pagination.after.is_none());
        assert!(!pagination.include_count);
    }

    #[test]
    fn cursor_pagination_new() {
        let pagination = CursorPagination::new(25);
        assert_eq!(pagination.limit, 25);
        assert!(pagination.after.is_none());
        assert!(!pagination.include_count);
    }

    #[test]
    fn cursor_pagination_with_count() {
        let pagination = CursorPagination::new(25).with_count();
        assert!(pagination.include_count);
    }

    #[test]
    fn cursor_pagination_limit_bounds() {
        let pagination = CursorPagination::new(0);
        assert_eq!(pagination.limit, 1);

        let pagination = CursorPagination::new(200);
        assert_eq!(pagination.limit, MAX_LIMIT);
    }

    #[test]
    fn cursor_pagination_fetch_limit() {
        let pagination = CursorPagination::new(50);
        assert_eq!(pagination.fetch_limit(), 51);
    }

    #[test]
    fn cursor_page_with_more() {
        let items: Vec<i32> = (1..=51).collect(); // 51 items = has more
        let page = CursorPage::new(items, Some(100), 50, |_| (Timestamp::now(), Uuid::new_v4()));

        assert_eq!(page.items.len(), 50);
        assert_eq!(page.total, Some(100));
        assert!(page.has_more());
        assert!(page.next_cursor.is_some());
    }

    #[test]
    fn cursor_page_without_more() {
        let items: Vec<i32> = (1..=30).collect(); // 30 items = no more
        let page = CursorPage::new(items, Some(30), 50, |_| (Timestamp::now(), Uuid::new_v4()));

        assert_eq!(page.items.len(), 30);
        assert_eq!(page.total, Some(30));
        assert!(!page.has_more());
        assert!(page.next_cursor.is_none());
    }

    #[test]
    fn cursor_page_without_count() {
        let items: Vec<i32> = (1..=30).collect();
        let page = CursorPage::new(items, None, 50, |_| (Timestamp::now(), Uuid::new_v4()));

        assert_eq!(page.total, None);
    }
}
