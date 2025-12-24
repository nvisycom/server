//! Pagination request types with performance and security considerations.
//!
//! This module provides pagination utilities that balance usability with performance
//! and security. It includes both offset-based and cursor-based pagination support
//! with comprehensive validation to prevent expensive database queries.

use nvisy_postgres::query::Pagination as QueryPagination;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use validator::Validate;

/// Pagination parameters with performance and security validation.
///
/// `Pagination` allows clients to retrieve data in chunks, which helps manage
/// large datasets by specifying how many records to skip and how many to fetch.
#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema, Validate)]
pub struct Pagination {
    /// The number of records to skip before starting to return results.
    ///
    /// For performance reasons, this is limited to prevent expensive deep
    /// pagination queries. Consider using cursor-based pagination for
    /// better performance when dealing with large datasets.
    ///
    /// **Performance Impact**: High offsets require the database to scan
    /// and skip many records, which can be slow for large tables.
    #[validate(range(min = 0, max = 100000))]
    pub offset: Option<u32>,

    /// The maximum number of records to return in a single request.
    ///
    /// This is balanced between usability and performance. Very large limits
    /// can cause memory pressure and slow response times.
    #[validate(range(min = 1, max = 1000))]
    pub limit: Option<u32>,
}

impl Pagination {
    /// Default pagination limit.
    const DEFAULT_LIMIT: u32 = 10;
    /// Default pagination offset.
    const DEFAULT_OFFSET: u32 = 0;

    /// Returns a new [`Pagination`].
    #[inline]
    pub fn new(offset: u32, limit: u32) -> Self {
        Self {
            offset: Some(offset),
            limit: Some(limit),
        }
    }

    /// Returns a [`Pagination`] with the given offset.
    #[inline]
    pub fn with_offset(mut self, offset: u32) -> Self {
        self.offset = Some(offset);
        self
    }

    /// Returns a [`Pagination`] with the given limit.
    #[inline]
    pub fn with_limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Returns the pagination offset.
    pub fn offset(&self) -> u32 {
        self.offset.unwrap_or(Self::DEFAULT_OFFSET)
    }

    /// Returns the pagination limit.
    pub fn limit(&self) -> u32 {
        self.limit.unwrap_or(Self::DEFAULT_LIMIT)
    }
}

impl From<Pagination> for QueryPagination {
    fn from(pagination: Pagination) -> Self {
        Self {
            offset: pagination.offset() as i64,
            limit: pagination.limit() as i64,
        }
    }
}
