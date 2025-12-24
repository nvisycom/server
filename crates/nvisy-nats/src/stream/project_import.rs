//! Project import job types and shared structures.

use std::time::Duration;

use jiff::Timestamp;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::event::{EventPriority, EventStatus};

/// Project import job payload
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct ProjectImportPayload {
    pub project_id: Uuid,
    pub account_id: Uuid,
    pub source_type: String,
    pub source_url: Option<String>,
    pub import_options: serde_json::Value,
}

/// Project import job
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct ProjectImportJob {
    pub id: Uuid,
    pub payload: ProjectImportPayload,
    pub priority: EventPriority,
    pub max_retries: u32,
    pub retry_count: u32,
    pub timeout: Duration,
    pub created_at: Timestamp,
    pub scheduled_for: Option<Timestamp>,
    pub status: EventStatus,
}

impl ProjectImportJob {
    /// Create a new project import job
    pub fn new(payload: ProjectImportPayload) -> Self {
        Self {
            id: Uuid::new_v4(),
            payload,
            priority: EventPriority::Normal,
            max_retries: 3,
            retry_count: 0,
            timeout: Duration::from_secs(600), // 10 minutes for imports
            created_at: Timestamp::now(),
            scheduled_for: None,
            status: EventStatus::Pending,
        }
    }

    /// Set job priority
    pub fn with_priority(mut self, priority: EventPriority) -> Self {
        self.priority = priority;
        self
    }

    /// Set maximum retries
    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }

    /// Set job timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Schedule job for later execution
    pub fn scheduled_for(mut self, timestamp: Timestamp) -> Self {
        self.scheduled_for = Some(timestamp);
        self
    }

    /// Check if job can be retried
    pub fn can_retry(&self) -> bool {
        self.retry_count < self.max_retries
    }

    /// Increment retry count
    pub fn increment_retry(&mut self) {
        self.retry_count += 1;
    }

    /// Check if job is ready to execute
    pub fn is_ready(&self) -> bool {
        self.scheduled_for
            .map(|scheduled| Timestamp::now() >= scheduled)
            .unwrap_or(true)
    }

    /// Get job age
    pub fn age(&self) -> Duration {
        let now = Timestamp::now();
        let signed_dur = now.duration_since(self.created_at);
        Duration::from_secs(signed_dur.as_secs().max(0) as u64)
    }

    /// Get the project ID
    pub fn project_id(&self) -> Uuid {
        self.payload.project_id
    }

    /// Get the account ID
    pub fn account_id(&self) -> Uuid {
        self.payload.account_id
    }
}
