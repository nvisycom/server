//! Keyset (cursor) pagination for database queries.
//!
//! Cursor pagination is stable and constant-time regardless of page depth: a
//! query orders by a **keyset** — a tuple of columns whose combined value is
//! unique and monotonic (a timestamp plus a tiebreaking id) — and the next page
//! is the rows strictly after the last one seen under that same order.
//!
//! The keyset is defined once per query as a [`CursorKey`] type `K`. The same
//! `K` drives all three things that must agree — the `ORDER BY`, the keyset
//! `WHERE` comparison, and the opaque cursor the client echoes back — so they
//! cannot silently disagree. The [`keyset`](crate::keyset) macro applies the
//! order, comparison, and limit to a query from a `K`'s columns and a
//! [`Direction`](crate::types::Direction) direction, eliminating the hand-written comparison at each call site.

use base64::prelude::*;
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::types::Direction;

/// Maximum number of items per page.
pub const MAX_LIMIT: i64 = 100;

/// A keyset: the ordered column values that position a row in a paginated query.
///
/// Implementors are plain, serializable structs of the ordering columns (e.g.
/// `{ created_at, id }`), most-significant field first. `Serialize` /
/// `DeserializeOwned` give the opaque wire cursor; the database does the actual
/// ordering, so no `Ord` is required here.
pub trait CursorKey: Serialize + DeserializeOwned {}

impl<K> CursorKey for K where K: Serialize + DeserializeOwned {}

/// An opaque position in a keyset-paginated result set: the [`CursorKey`] of the
/// last row of the previous page.
///
/// Serializes to and from a URL-safe base64 string of the key's JSON, so the
/// client treats it as an opaque token and the wire form is not tied to any
/// particular key shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cursor<K: CursorKey> {
    /// The keyset of the last row seen.
    pub key: K,
}

impl<K: CursorKey> Cursor<K> {
    /// Wraps a keyset as a cursor.
    pub fn new(key: K) -> Self {
        Self { key }
    }

    /// Encodes the cursor as a URL-safe base64 string of the key's JSON.
    pub fn encode(&self) -> String {
        // The key is a small fixed struct, so serialization cannot realistically
        // fail; fall back to an empty token rather than panicking in a getter.
        let json = serde_json::to_vec(&self.key).unwrap_or_default();
        BASE64_URL_SAFE_NO_PAD.encode(json)
    }

    /// Decodes a cursor from a URL-safe base64 string, or `None` if it is not a
    /// valid encoding of this key type.
    pub fn decode(encoded: &str) -> Option<Self> {
        let json = BASE64_URL_SAFE_NO_PAD.decode(encoded).ok()?;
        let key = serde_json::from_slice(&json).ok()?;
        Some(Self { key })
    }
}

/// Keyset-pagination parameters: how many rows, from where, in which direction,
/// and whether to also count the total.
///
/// Generic over the query's [`CursorKey`] so `after` is a typed cursor decoded
/// against exactly that key.
#[derive(Debug, Clone)]
pub struct CursorPagination<K: CursorKey> {
    /// Maximum rows to return (clamped to `1..=MAX_LIMIT`).
    pub limit: i64,
    /// The cursor to continue after; `None` starts at the first page.
    pub after: Option<Cursor<K>>,
    /// Whether to also run the total-count query (skipped by default).
    pub include_count: bool,
    /// The order to walk rows in.
    pub direction: Direction,
}

impl<K: CursorKey> CursorPagination<K> {
    /// A first page of `limit` rows, newest first, without a count.
    pub fn new(limit: i64) -> Self {
        Self {
            limit: limit.clamp(1, MAX_LIMIT),
            after: None,
            include_count: false,
            direction: Direction::Descending,
        }
    }

    /// Decodes an optional encoded cursor string into typed pagination. An invalid
    /// cursor string is treated as no cursor (the first page).
    pub fn from_cursor_string(limit: i64, cursor: Option<&str>) -> Self {
        Self {
            limit: limit.clamp(1, MAX_LIMIT),
            after: cursor.and_then(Cursor::decode),
            include_count: false,
            direction: Direction::Descending,
        }
    }

    /// Sets the walk direction.
    #[must_use]
    pub fn with_direction(mut self, direction: Direction) -> Self {
        self.direction = direction;
        self
    }

    /// Enables the total-count query.
    #[must_use]
    pub fn with_count(mut self) -> Self {
        self.include_count = true;
        self
    }

    /// The key to compare against, when continuing after a cursor.
    pub fn after_key(&self) -> Option<&K> {
        self.after.as_ref().map(|c| &c.key)
    }

    /// The fetch limit: one more than `limit`, so a full extra row signals that a
    /// further page exists (it is dropped before the page is returned).
    pub fn fetch_limit(&self) -> i64 {
        self.limit + 1
    }
}

/// A page of keyset-paginated rows: the items, an optional total, and the opaque
/// cursor for the next page (present only when more rows exist).
#[derive(Debug, Clone)]
pub struct CursorPage<T> {
    /// The rows in this page.
    pub items: Vec<T>,
    /// Total rows matching the query, when a count was requested.
    pub total: Option<i64>,
    /// The cursor for the next page, or `None` at the end.
    pub next_cursor: Option<String>,
}

impl<T> CursorPage<T> {
    /// Builds a page from a fetched batch (of up to `limit + 1` rows).
    ///
    /// If the batch holds more than `limit` rows, the extra one is dropped and its
    /// predecessor's [`CursorKey`] (from `cursor_fn`) becomes the next cursor;
    /// otherwise this is the last page. `total` is passed through.
    pub fn new<K, F>(mut items: Vec<T>, total: Option<i64>, limit: i64, cursor_fn: F) -> Self
    where
        K: CursorKey,
        F: Fn(&T) -> K,
    {
        let has_more = items.len() as i64 > limit;
        if has_more {
            items.pop();
        }
        let next_cursor = has_more
            .then(|| {
                items
                    .last()
                    .map(|item| Cursor::new(cursor_fn(item)).encode())
            })
            .flatten();
        Self {
            items,
            total,
            next_cursor,
        }
    }

    /// An empty page (no rows, count zero, no next cursor).
    pub fn empty() -> Self {
        Self {
            items: Vec::new(),
            total: Some(0),
            next_cursor: None,
        }
    }

    /// Whether a further page exists.
    pub fn has_more(&self) -> bool {
        self.next_cursor.is_some()
    }

    /// Maps the items to another type, keeping the total and next cursor.
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

/// Applies keyset ordering, the after-cursor comparison, and the fetch limit to a
/// boxed Diesel query, in one place, so no call site hand-writes the comparison.
///
/// `keyset!(query, sort_col, id_col, direction, after)` orders by
/// `(sort_col, id_col)` in `direction`, filters to the rows strictly after
/// `after` (an `Option<(sort_value, id_value)>` read from the decoded cursor key),
/// and limits to `limit`. It returns the boxed query, ready for
/// `.select(...).limit(..).load(..)`.
///
/// Generic over the sort column's type: `sort_col` may be any orderable column
/// (a timestamp, a text field, …) and `after`'s first element is its comparable
/// value — nothing here assumes a timestamp. `direction` is a
/// [`Direction`](crate::types::Direction); `after` is typically
/// `pagination.after_key().map(|k| (k.field.into(), k.id))`.
macro_rules! keyset {
    ($query:expr, $sort:expr, $id:expr, $direction:expr, $after:expr) => {{
        use $crate::types::Direction;
        let mut query = $query;

        // Continue strictly after the cursor's row, in the walk direction: for
        // Descending, rows before the sort value, or equal on it and a smaller id;
        // for Ascending, the mirror.
        if let Some((after_sort, after_id)) = $after {
            query = match $direction {
                Direction::Descending => query.filter(
                    $sort
                        .lt(after_sort.clone())
                        .or($sort.eq(after_sort).and($id.lt(after_id))),
                ),
                Direction::Ascending => query.filter(
                    $sort
                        .gt(after_sort.clone())
                        .or($sort.eq(after_sort).and($id.gt(after_id))),
                ),
            };
        }

        query = match $direction {
            Direction::Descending => query.order(($sort.desc(), $id.desc())),
            Direction::Ascending => query.order(($sort.asc(), $id.asc())),
        };

        query
    }};
}

pub(crate) use keyset;

#[cfg(test)]
mod tests {
    use jiff::Timestamp;
    use serde::Deserialize;
    use uuid::Uuid;

    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    struct TimeKey {
        created_at: Timestamp,
        id: Uuid,
    }

    #[test]
    fn cursor_encode_decode_roundtrip() {
        let key = TimeKey {
            created_at: Timestamp::now(),
            id: Uuid::new_v4(),
        };
        let cursor = Cursor::new(key.clone());
        let decoded = Cursor::<TimeKey>::decode(&cursor.encode()).expect("decode");
        assert_eq!(decoded.key, key);
    }

    #[test]
    fn cursor_decode_rejects_garbage() {
        assert!(Cursor::<TimeKey>::decode("not base64!!").is_none());
        assert!(Cursor::<TimeKey>::decode("").is_none());
        // Valid base64 of the wrong shape does not decode to this key.
        let wrong = BASE64_URL_SAFE_NO_PAD.encode(b"{}");
        assert!(Cursor::<TimeKey>::decode(&wrong).is_none());
    }

    #[test]
    fn pagination_clamps_limit_and_defaults_descending() {
        assert_eq!(CursorPagination::<TimeKey>::new(0).limit, 1);
        assert_eq!(CursorPagination::<TimeKey>::new(500).limit, MAX_LIMIT);
        assert_eq!(
            CursorPagination::<TimeKey>::new(10).direction,
            Direction::Descending
        );
        assert_eq!(CursorPagination::<TimeKey>::new(10).fetch_limit(), 11);
    }

    #[test]
    fn page_drops_the_probe_row_and_sets_a_next_cursor() {
        // limit 2, three rows fetched (the probe) -> two returned + a next cursor.
        let rows = vec![
            TimeKey {
                created_at: Timestamp::now(),
                id: Uuid::new_v4(),
            },
            TimeKey {
                created_at: Timestamp::now(),
                id: Uuid::new_v4(),
            },
            TimeKey {
                created_at: Timestamp::now(),
                id: Uuid::new_v4(),
            },
        ];
        let page = CursorPage::new(rows, None, 2, |k: &TimeKey| k.clone());
        assert_eq!(page.items.len(), 2);
        assert!(page.next_cursor.is_some());

        // A short batch is the last page.
        let last = CursorPage::new(vec![1_i32, 2], Some(2), 5, |n: &i32| TimeKey {
            created_at: Timestamp::now(),
            id: Uuid::from_u128(u128::try_from(*n).unwrap()),
        });
        assert_eq!(last.items.len(), 2);
        assert!(last.next_cursor.is_none());
        assert_eq!(last.total, Some(2));
    }
}
