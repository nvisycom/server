use nvisy_postgres::query::Pagination as QueryPagination;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Represents pagination parameters commonly used in API queries.
///
/// `Pagination` allows clients to retrieve data in chunks, which helps manage
/// large datasets by specifying how many records to skip and how many to fetch.
#[derive(Debug, Default, Copy, Clone, Serialize, Deserialize, ToSchema)]
pub struct PaginationRequest {
    /// The number of records to skip before starting to return results.
    ///
    /// Useful for implementing paged responses where you want to skip a
    /// certain number of entries (e.g., skip the first 10).
    pub offset: Option<u32>,

    /// The maximum number of records to return.
    ///
    /// This limits the number of items retrieved in a single request,
    /// commonly used to cap the response size (e.g., return only 25 items).
    pub limit: Option<u32>,
}

impl PaginationRequest {
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

impl From<PaginationRequest> for QueryPagination {
    fn from(pagination: PaginationRequest) -> Self {
        Self {
            limit: pagination.limit() as i64,
            offset: pagination.offset() as i64,
        }
    }
}
