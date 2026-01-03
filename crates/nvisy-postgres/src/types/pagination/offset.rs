//! Offset-based pagination for database queries.
//!
//! Offset pagination is suitable for small datasets or when random page access
//! is required. For large datasets, prefer cursor-based pagination.

#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Maximum number of items per page.
pub const MAX_LIMIT: i64 = 1000;

/// Offset-based pagination parameters for database queries.
///
/// Use this for admin dashboards or when users need to jump to specific pages.
/// For infinite scroll or API iteration, prefer [`CursorPagination`].
///
/// [`CursorPagination`]: super::CursorPagination
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct OffsetPagination {
    /// Maximum number of records to return.
    pub limit: i64,
    /// Number of records to skip.
    pub offset: i64,
    /// Whether to include total count in the response.
    /// Set to `false` to skip the count query for better performance.
    #[serde(default)]
    pub include_count: bool,
}

impl OffsetPagination {
    /// Creates a new pagination instance.
    pub fn new(limit: i64, offset: i64) -> Self {
        Self {
            limit: limit.clamp(1, MAX_LIMIT),
            offset: offset.max(0),
            include_count: false,
        }
    }

    /// Creates pagination from page number and page size.
    pub fn from_page(page: i64, page_size: i64) -> Self {
        let page = page.max(1);
        let page_size = page_size.clamp(1, MAX_LIMIT);
        Self {
            limit: page_size,
            offset: (page - 1) * page_size,
            include_count: false,
        }
    }

    /// Enables including total count in the response.
    pub fn with_count(mut self) -> Self {
        self.include_count = true;
        self
    }

    /// Gets the current page number (1-based).
    pub fn page_number(&self) -> i64 {
        (self.offset / self.limit) + 1
    }

    /// Gets the page size.
    pub fn page_size(&self) -> i64 {
        self.limit
    }
}

impl Default for OffsetPagination {
    fn default() -> Self {
        Self {
            limit: 50,
            offset: 0,
            include_count: false,
        }
    }
}

/// Result of an offset-paginated query.
#[derive(Debug, Clone)]
pub struct OffsetPage<T> {
    /// The items in this page.
    pub items: Vec<T>,
    /// Total count of items matching the query (across all pages).
    /// Only present if `include_count` was set in the pagination request.
    pub total: Option<i64>,
}

impl<T> OffsetPage<T> {
    /// Creates a new offset page.
    pub fn new(items: Vec<T>, total: Option<i64>) -> Self {
        Self { items, total }
    }

    /// Creates an empty offset page.
    pub fn empty() -> Self {
        Self {
            items: Vec::new(),
            total: Some(0),
        }
    }

    /// Maps the items to a different type.
    pub fn map<U, F>(self, f: F) -> OffsetPage<U>
    where
        F: FnMut(T) -> U,
    {
        OffsetPage {
            items: self.items.into_iter().map(f).collect(),
            total: self.total,
        }
    }

    /// Returns whether there are more pages after this one.
    ///
    /// Requires `total` to be present.
    pub fn has_more(&self, pagination: &OffsetPagination) -> Option<bool> {
        self.total
            .map(|total| (pagination.offset + self.items.len() as i64) < total)
    }

    /// Returns the total number of pages.
    ///
    /// Requires `total` to be present.
    pub fn total_pages(&self, pagination: &OffsetPagination) -> Option<i64> {
        self.total
            .map(|total| (total + pagination.limit - 1) / pagination.limit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pagination_new() {
        let pagination = OffsetPagination::new(25, 100);
        assert_eq!(pagination.limit, 25);
        assert_eq!(pagination.offset, 100);
        assert!(!pagination.include_count);
    }

    #[test]
    fn pagination_with_count() {
        let pagination = OffsetPagination::new(25, 100).with_count();
        assert!(pagination.include_count);
    }

    #[test]
    fn pagination_bounds_checking() {
        // Test limit bounds
        let pagination = OffsetPagination::new(0, 10);
        assert_eq!(pagination.limit, 1);

        let pagination = OffsetPagination::new(1500, 10);
        assert_eq!(pagination.limit, MAX_LIMIT);

        // Test offset bounds
        let pagination = OffsetPagination::new(10, -5);
        assert_eq!(pagination.offset, 0);
    }

    #[test]
    fn pagination_from_page() {
        let pagination = OffsetPagination::from_page(1, 20);
        assert_eq!(pagination.limit, 20);
        assert_eq!(pagination.offset, 0);

        let pagination = OffsetPagination::from_page(2, 20);
        assert_eq!(pagination.limit, 20);
        assert_eq!(pagination.offset, 20);

        let pagination = OffsetPagination::from_page(3, 10);
        assert_eq!(pagination.limit, 10);
        assert_eq!(pagination.offset, 20);

        let pagination = OffsetPagination::from_page(0, 20);
        assert_eq!(pagination.offset, 0);

        let pagination = OffsetPagination::from_page(1, 0);
        assert_eq!(pagination.limit, 1);
    }

    #[test]
    fn pagination_page_number() {
        let pagination = OffsetPagination::new(20, 0);
        assert_eq!(pagination.page_number(), 1);

        let pagination = OffsetPagination::new(20, 20);
        assert_eq!(pagination.page_number(), 2);

        let pagination = OffsetPagination::new(10, 25);
        assert_eq!(pagination.page_number(), 3);

        let pagination = OffsetPagination::new(15, 30);
        assert_eq!(pagination.page_number(), 3);
    }

    #[test]
    fn offset_page_has_more() {
        let pagination = OffsetPagination::new(10, 0);
        let page = OffsetPage::new(vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10], Some(25));
        assert_eq!(page.has_more(&pagination), Some(true));

        let page = OffsetPage::new(vec![1, 2, 3, 4, 5], Some(5));
        assert_eq!(page.has_more(&pagination), Some(false));

        let page: OffsetPage<i32> = OffsetPage::new(vec![], None);
        assert_eq!(page.has_more(&pagination), None);
    }

    #[test]
    fn offset_page_total_pages() {
        let pagination = OffsetPagination::new(10, 0);

        let page: OffsetPage<i32> = OffsetPage::new(vec![], Some(25));
        assert_eq!(page.total_pages(&pagination), Some(3));

        let page: OffsetPage<i32> = OffsetPage::new(vec![], Some(30));
        assert_eq!(page.total_pages(&pagination), Some(3));

        let page: OffsetPage<i32> = OffsetPage::new(vec![], Some(31));
        assert_eq!(page.total_pages(&pagination), Some(4));

        let page: OffsetPage<i32> = OffsetPage::new(vec![], None);
        assert_eq!(page.total_pages(&pagination), None);
    }
}
