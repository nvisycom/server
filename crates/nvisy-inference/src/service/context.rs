//! Context and usage tracking for inference operations.

use std::ops::{Add, AddAssign};

use jiff::{SignedDuration, Timestamp};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Context information for a single inference request.
///
/// Each request should have its own context instance containing
/// the account and workspace identifiers for billing and isolation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Context {
    /// Unique identifier for this context/request.
    pub context_id: Uuid,
    /// Context creation timestamp.
    pub created_at: Timestamp,
    /// Account identifier (optional, for anonymous requests).
    pub account_id: Option<Uuid>,
    /// Workspace identifier (required for all requests).
    pub workspace_id: Uuid,
}

impl Context {
    /// Create a new context with the required workspace ID.
    pub fn new(workspace_id: Uuid) -> Self {
        Self {
            context_id: Uuid::now_v7(),
            created_at: Timestamp::now(),
            account_id: None,
            workspace_id,
        }
    }

    /// Create a new context with account and workspace IDs.
    pub fn with_account(account_id: Uuid, workspace_id: Uuid) -> Self {
        Self {
            context_id: Uuid::now_v7(),
            created_at: Timestamp::now(),
            account_id: Some(account_id),
            workspace_id,
        }
    }

    /// Set the account ID.
    pub fn set_account_id(&mut self, account_id: Uuid) {
        self.account_id = Some(account_id);
    }

    /// Get the context ID.
    pub fn context_id(&self) -> Uuid {
        self.context_id
    }

    /// Get the account ID if set.
    pub fn account_id(&self) -> Option<Uuid> {
        self.account_id
    }

    /// Get the workspace ID.
    pub fn workspace_id(&self) -> Uuid {
        self.workspace_id
    }

    /// Get the context creation timestamp.
    pub fn created_at(&self) -> Timestamp {
        self.created_at
    }

    /// Calculate elapsed time since context creation.
    pub fn elapsed(&self) -> SignedDuration {
        Timestamp::now().duration_since(self.created_at)
    }
}

/// Usage statistics for provider operations.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct UsageStats {
    /// Total tokens processed.
    pub total_tokens: u32,
    /// Total runs (embeddings generated, images processed, pages processed, etc.).
    pub total_runs: u32,
    /// Total processing time.
    pub total_processing_time: SignedDuration,
    /// Number of successful requests.
    pub successful_requests: u32,
    /// Number of failed requests.
    pub failed_requests: u32,
}

impl UsageStats {
    /// Create a new empty usage stats.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create usage stats for a successful request.
    pub fn success(tokens: u32, runs: u32, processing_time: SignedDuration) -> Self {
        Self {
            total_tokens: tokens,
            total_runs: runs,
            total_processing_time: processing_time,
            successful_requests: 1,
            failed_requests: 0,
        }
    }

    /// Create usage stats for a failed request.
    pub fn failure(tokens: u32, processing_time: SignedDuration) -> Self {
        Self {
            total_tokens: tokens,
            total_runs: 0,
            total_processing_time: processing_time,
            successful_requests: 0,
            failed_requests: 1,
        }
    }

    /// Builder method to set total tokens.
    pub fn with_tokens(mut self, tokens: u32) -> Self {
        self.total_tokens = tokens;
        self
    }

    /// Builder method to set total runs.
    pub fn with_runs(mut self, runs: u32) -> Self {
        self.total_runs = runs;
        self
    }

    /// Builder method to set processing time.
    pub fn with_processing_time(mut self, processing_time: SignedDuration) -> Self {
        self.total_processing_time = processing_time;
        self
    }

    /// Builder method to set successful requests count.
    pub fn with_successful_requests(mut self, count: u32) -> Self {
        self.successful_requests = count;
        self
    }

    /// Builder method to set failed requests count.
    pub fn with_failed_requests(mut self, count: u32) -> Self {
        self.failed_requests = count;
        self
    }

    /// Get total number of requests (successful + failed).
    pub fn total_requests(&self) -> u32 {
        self.successful_requests + self.failed_requests
    }

    /// Calculate success rate as a percentage (0.0 to 100.0).
    pub fn success_rate(&self) -> f32 {
        let total = self.total_requests();
        if total == 0 {
            0.0
        } else {
            (self.successful_requests as f32 / total as f32) * 100.0
        }
    }

    /// Calculate failure rate as a percentage (0.0 to 100.0).
    pub fn failure_rate(&self) -> f32 {
        let total = self.total_requests();
        if total == 0 {
            0.0
        } else {
            (self.failed_requests as f32 / total as f32) * 100.0
        }
    }

    /// Calculate average processing time per request.
    pub fn average_processing_time(&self) -> Option<SignedDuration> {
        let total = self.total_requests();
        if total == 0 {
            None
        } else {
            Some(self.total_processing_time / total as i32)
        }
    }

    /// Calculate average tokens per request.
    pub fn average_tokens_per_request(&self) -> Option<f32> {
        let total = self.total_requests();
        if total == 0 {
            None
        } else {
            Some(self.total_tokens as f32 / total as f32)
        }
    }

    /// Calculate average runs per successful request.
    pub fn average_runs_per_request(&self) -> Option<f32> {
        if self.successful_requests == 0 {
            None
        } else {
            Some(self.total_runs as f32 / self.successful_requests as f32)
        }
    }

    /// Check if there's any usage data.
    pub fn has_usage(&self) -> bool {
        self.total_requests() > 0
    }

    /// Check if all requests were successful.
    pub fn all_successful(&self) -> bool {
        self.failed_requests == 0 && self.successful_requests > 0
    }

    /// Check if all requests failed.
    pub fn all_failed(&self) -> bool {
        self.successful_requests == 0 && self.failed_requests > 0
    }

    /// Reset all statistics to zero.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Merge another UsageStats into this one.
    pub fn merge(&mut self, other: &UsageStats) {
        self.total_tokens += other.total_tokens;
        self.total_runs += other.total_runs;
        self.total_processing_time = self
            .total_processing_time
            .checked_add(other.total_processing_time)
            .unwrap_or(self.total_processing_time);
        self.successful_requests += other.successful_requests;
        self.failed_requests += other.failed_requests;
    }
}

impl Add for UsageStats {
    type Output = Self;

    fn add(mut self, rhs: Self) -> Self::Output {
        self.merge(&rhs);
        self
    }
}

impl AddAssign for UsageStats {
    fn add_assign(&mut self, rhs: Self) {
        self.merge(&rhs);
    }
}
