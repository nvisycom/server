//! Project run model for PostgreSQL database operations.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::project_runs;
use crate::types::{HasCreatedAt, HasUpdatedAt, IntegrationStatus};

/// Project run model representing integration run tracking.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = project_runs)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct ProjectRun {
    /// Unique run identifier.
    pub id: Uuid,
    /// Reference to the project this run belongs to.
    pub project_id: Uuid,
    /// Reference to the integration (NULL for manual runs).
    pub integration_id: Option<Uuid>,
    /// Account that triggered the run (NULL for automated runs).
    pub account_id: Option<Uuid>,
    /// Human-readable run name.
    pub run_name: String,
    /// Type of run (manual, scheduled, triggered, etc.).
    pub run_type: String,
    /// Current run status.
    pub run_status: IntegrationStatus,
    /// Timestamp when run execution started.
    pub started_at: Option<Timestamp>,
    /// Timestamp when run execution completed.
    pub completed_at: Option<Timestamp>,
    /// Run duration in milliseconds.
    pub duration_ms: Option<i32>,
    /// Summary of run results.
    pub result_summary: Option<String>,
    /// Run metadata and configuration.
    pub metadata: serde_json::Value,
    /// Error details for failed runs.
    pub error_details: Option<serde_json::Value>,
    /// Timestamp when the run was created.
    pub created_at: Timestamp,
    /// Timestamp when the run was last updated.
    pub updated_at: Timestamp,
}

/// Data for creating a new project run.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = project_runs)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewProjectRun {
    /// Project ID.
    pub project_id: Uuid,
    /// Integration ID.
    pub integration_id: Option<Uuid>,
    /// Account ID.
    pub account_id: Option<Uuid>,
    /// Run name.
    pub run_name: Option<String>,
    /// Run type.
    pub run_type: Option<String>,
    /// Run status.
    pub run_status: Option<IntegrationStatus>,
    /// Metadata.
    pub metadata: Option<serde_json::Value>,
}

/// Data for updating a project run.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = project_runs)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UpdateProjectRun {
    /// Run name.
    pub run_name: Option<String>,
    /// Run status.
    pub run_status: Option<IntegrationStatus>,
    /// Started at.
    pub started_at: Option<Option<Timestamp>>,
    /// Completed at.
    pub completed_at: Option<Option<Timestamp>>,
    /// Duration in milliseconds.
    pub duration_ms: Option<Option<i32>>,
    /// Result summary.
    pub result_summary: Option<Option<String>>,
    /// Metadata.
    pub metadata: Option<serde_json::Value>,
    /// Error details.
    pub error_details: Option<Option<serde_json::Value>>,
}

impl ProjectRun {
    /// Returns whether the run was created recently.
    pub fn is_recent(&self) -> bool {
        self.was_created_within(jiff::Span::new().hours(24))
    }

    /// Returns whether the run is pending.
    pub fn is_pending(&self) -> bool {
        matches!(self.run_status, IntegrationStatus::Pending)
    }

    /// Returns whether the run is currently executing.
    pub fn is_executing(&self) -> bool {
        matches!(self.run_status, IntegrationStatus::Executing)
    }

    /// Returns whether the run has failed.
    pub fn has_failed(&self) -> bool {
        matches!(self.run_status, IntegrationStatus::Failure)
    }

    /// Returns whether the run is complete (either failed or finished).
    pub fn is_complete(&self) -> bool {
        self.completed_at.is_some()
    }

    /// Returns whether the run is in progress.
    pub fn is_in_progress(&self) -> bool {
        self.started_at.is_some() && self.completed_at.is_none()
    }

    /// Returns whether this is a manual run.
    pub fn is_manual(&self) -> bool {
        self.run_type.eq_ignore_ascii_case("manual")
    }

    /// Returns whether this is an automated run.
    pub fn is_automated(&self) -> bool {
        self.account_id.is_none()
    }

    /// Returns whether the run has error details.
    pub fn has_errors(&self) -> bool {
        self.error_details
            .as_ref()
            .is_some_and(|e| !e.as_object().is_none_or(|obj| obj.is_empty()))
    }

    /// Returns whether the run has custom metadata.
    pub fn has_metadata(&self) -> bool {
        !self.metadata.as_object().is_none_or(|obj| obj.is_empty())
    }

    /// Returns the duration as a jiff_diesel::Span if available.
    pub fn duration(&self) -> Option<jiff::Span> {
        self.duration_ms
            .map(|ms| jiff::Span::new().milliseconds(ms as i64))
    }

    /// Returns the duration in a human-readable format.
    pub fn duration_human(&self) -> Option<String> {
        self.duration_ms.map(|ms| {
            let seconds = ms / 1000;
            let minutes = seconds / 60;
            let hours = minutes / 60;

            if hours > 0 {
                format!("{}h {}m", hours, minutes % 60)
            } else if minutes > 0 {
                format!("{}m {}s", minutes, seconds % 60)
            } else {
                format!("{}s", seconds)
            }
        })
    }

    /// Returns the elapsed time if run is in progress.
    pub fn elapsed_time(&self) -> Option<jiff::Span> {
        if let Some(started) = self.started_at {
            if self.completed_at.is_none() {
                Some(jiff::Timestamp::now() - jiff::Timestamp::from(started))
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Returns whether the run exceeded a specific duration.
    pub fn exceeded_duration(&self, max_duration_ms: i32) -> bool {
        self.duration_ms.is_some_and(|d| d > max_duration_ms)
    }

    /// Returns whether this is a specific run type.
    pub fn is_type(&self, type_name: &str) -> bool {
        self.run_type.eq_ignore_ascii_case(type_name)
    }
}

impl HasCreatedAt for ProjectRun {
    fn created_at(&self) -> jiff::Timestamp {
        self.created_at.into()
    }
}

impl HasUpdatedAt for ProjectRun {
    fn updated_at(&self) -> jiff::Timestamp {
        self.updated_at.into()
    }
}
