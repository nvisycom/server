//! Database query repositories for all entities in the system.
//!
//! This module contains repository implementations that provide high-level
//! database operations for all entities, encapsulating common patterns
//! and providing type-safe interfaces.
//!
//! # Pagination
//!
//! All queries that may return large result sets use the [`Pagination`] struct
//! to provide consistent, bounded pagination across the system.

pub mod account;
pub mod account_action_tokens;
pub mod account_api_tokens;
pub mod document;
pub mod document_files;
pub mod document_versions;

pub mod project;
pub mod project_activity;
pub mod project_integration;
pub mod project_invite;
pub mod project_member;

pub use account::{AccountRepository, AccountStatistics};
pub use account_action_tokens::{AccountActionTokenRepository, AccountActionTokenStats};
pub use account_api_tokens::{AccountApiTokenRepository, AccountApiTokenStats};
pub use document::{DocumentProjectStats, DocumentRepository, DocumentUserStats};
pub use document_files::{DocumentFileRepository, DocumentFileStats, ProcessingSummary};
pub use document_versions::{DocumentVersionRepository, DocumentVersionStats, UserVersionStats};
pub use project::ProjectRepository;
pub use project_activity::ProjectActivityRepository;
pub use project_integration::ProjectIntegrationRepository;
pub use project_invite::ProjectInviteRepository;
pub use project_member::ProjectMemberRepository;
use serde::{Deserialize, Serialize};

/// Pagination parameters for database queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pagination {
    /// Maximum number of records to return.
    pub limit: i64,
    /// Number of records to skip.
    pub offset: i64,
}

impl Pagination {
    /// Creates a new pagination instance.
    pub fn new(limit: i64, offset: i64) -> Self {
        Self {
            // Ensure limit is between 1 and 1000
            limit: limit.clamp(1, 1000),
            // Ensure offset is non-negative
            offset: offset.max(0),
        }
    }

    /// Creates pagination from page number and page size.
    pub fn from_page(page: i64, page_size: i64) -> Self {
        let page = page.max(1);
        let page_size = page_size.clamp(1, 1000);
        Self::new(page_size, (page - 1) * page_size)
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

impl Default for Pagination {
    fn default() -> Self {
        Self::new(50, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pagination_new() {
        let pagination = Pagination::new(25, 100);
        assert_eq!(pagination.limit, 25);
        assert_eq!(pagination.offset, 100);
    }

    #[test]
    fn pagination_bounds_checking() {
        // Test limit bounds
        let pagination = Pagination::new(0, 10);
        assert_eq!(pagination.limit, 1); // Should be clamped to minimum 1

        let pagination = Pagination::new(1500, 10);
        assert_eq!(pagination.limit, 1000); // Should be clamped to maximum 1000

        // Test offset bounds
        let pagination = Pagination::new(10, -5);
        assert_eq!(pagination.offset, 0); // Should be clamped to minimum 0
    }

    #[test]
    fn pagination_from_page() {
        // Test first page
        let pagination = Pagination::from_page(1, 20);
        assert_eq!(pagination.limit, 20);
        assert_eq!(pagination.offset, 0);

        // Test second page
        let pagination = Pagination::from_page(2, 20);
        assert_eq!(pagination.limit, 20);
        assert_eq!(pagination.offset, 20);

        // Test third page
        let pagination = Pagination::from_page(3, 10);
        assert_eq!(pagination.limit, 10);
        assert_eq!(pagination.offset, 20);

        // Test bounds checking
        let pagination = Pagination::from_page(0, 20); // Should be clamped to page 1
        assert_eq!(pagination.offset, 0);

        let pagination = Pagination::from_page(1, 0); // Should be clamped to page_size 1
        assert_eq!(pagination.limit, 1);
    }

    #[test]
    fn pagination_page_number() {
        let pagination = Pagination::new(20, 0);
        assert_eq!(pagination.page_number(), 1);

        let pagination = Pagination::new(20, 20);
        assert_eq!(pagination.page_number(), 2);

        let pagination = Pagination::new(10, 25);
        assert_eq!(pagination.page_number(), 3); // 25 / 10 + 1 = 3

        let pagination = Pagination::new(15, 30);
        assert_eq!(pagination.page_number(), 3); // 30 / 15 + 1 = 3
    }
}
