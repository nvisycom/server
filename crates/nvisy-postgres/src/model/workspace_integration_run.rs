//! Workspace run model for PostgreSQL database operations.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::workspace_integration_runs;
use crate::types::{IntegrationStatus, RunType};

/// Workspace run model representing integration run tracking.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = workspace_integration_runs)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct WorkspaceIntegrationRun {
    /// Unique run identifier.
    pub id: Uuid,
    /// Reference to the workspace this run belongs to.
    pub workspace_id: Uuid,
    /// Reference to the integration (NULL for manual runs).
    pub integration_id: Option<Uuid>,
    /// Account that triggered the run (NULL for automated runs).
    pub account_id: Option<Uuid>,
    /// Type of run (manual, scheduled, triggered).
    pub run_type: RunType,
    /// Current run status.
    pub run_status: IntegrationStatus,
    /// Run metadata, results, and error details.
    pub metadata: serde_json::Value,
    /// Run execution logs.
    pub logs: serde_json::Value,
    /// Timestamp when run was started.
    pub started_at: Timestamp,
    /// Timestamp when run was completed.
    pub completed_at: Option<Timestamp>,
}

/// Data for creating a new workspace run.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = workspace_integration_runs)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewWorkspaceIntegrationRun {
    /// Workspace ID.
    pub workspace_id: Uuid,
    /// Integration ID.
    pub integration_id: Option<Uuid>,
    /// Account ID.
    pub account_id: Option<Uuid>,
    /// Run type.
    pub run_type: Option<RunType>,
    /// Run status.
    pub run_status: Option<IntegrationStatus>,
    /// Metadata.
    pub metadata: Option<serde_json::Value>,
    /// Logs.
    pub logs: Option<serde_json::Value>,
}

/// Data for updating a workspace run.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = workspace_integration_runs)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UpdateWorkspaceIntegrationRun {
    /// Run status.
    pub run_status: Option<IntegrationStatus>,
    /// Metadata.
    pub metadata: Option<serde_json::Value>,
    /// Logs.
    pub logs: Option<serde_json::Value>,
    /// Completed at.
    pub completed_at: Option<Option<Timestamp>>,
}

impl WorkspaceIntegrationRun {
    /// Returns whether the run was started recently (within 24 hours).
    pub fn is_recent(&self) -> bool {
        let now = jiff::Timestamp::now();
        let started: jiff::Timestamp = self.started_at.into();
        now.since(started).is_ok_and(|span| span.get_hours() < 24)
    }

    /// Returns whether the run is pending.
    pub fn is_pending(&self) -> bool {
        matches!(self.run_status, IntegrationStatus::Pending)
    }

    /// Returns whether the run is currently running.
    pub fn is_running(&self) -> bool {
        matches!(self.run_status, IntegrationStatus::Running)
    }

    /// Returns whether the run has been cancelled.
    pub fn is_cancelled(&self) -> bool {
        matches!(self.run_status, IntegrationStatus::Cancelled)
    }

    /// Returns whether the run is complete.
    pub fn is_complete(&self) -> bool {
        self.completed_at.is_some()
    }

    /// Returns whether the run is in progress.
    pub fn is_in_progress(&self) -> bool {
        self.completed_at.is_none()
    }

    /// Returns whether this is a manual run.
    pub fn is_manual(&self) -> bool {
        self.run_type.is_manual()
    }

    /// Returns whether this is an automated run.
    pub fn is_automated(&self) -> bool {
        self.run_type.is_automated()
    }

    /// Returns whether the run has custom metadata.
    pub fn has_metadata(&self) -> bool {
        !self.metadata.as_object().is_none_or(|obj| obj.is_empty())
    }

    /// Returns the elapsed time if run is in progress.
    pub fn elapsed_time(&self) -> Option<jiff::Span> {
        if self.completed_at.is_none() {
            let started: jiff::Timestamp = self.started_at.into();
            Some(jiff::Timestamp::now() - started)
        } else {
            None
        }
    }
}
