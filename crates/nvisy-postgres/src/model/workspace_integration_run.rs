//! Workspace run model for PostgreSQL database operations.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::workspace_integration_runs;
use crate::types::{HasCreatedAt, HasUpdatedAt, IntegrationStatus};

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
    /// Human-readable run name.
    pub run_name: String,
    /// Type of run (manual, scheduled, triggered, etc.).
    pub run_type: String,
    /// Current run status.
    pub run_status: IntegrationStatus,
    /// Run metadata, results, and error details.
    pub metadata: serde_json::Value,
    /// Timestamp when run execution started.
    pub started_at: Option<Timestamp>,
    /// Timestamp when run execution completed.
    pub completed_at: Option<Timestamp>,
    /// Timestamp when the run was created.
    pub created_at: Timestamp,
    /// Timestamp when the run was last updated.
    pub updated_at: Timestamp,
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
    /// Run name.
    pub run_name: Option<String>,
    /// Run type.
    pub run_type: Option<String>,
    /// Run status.
    pub run_status: Option<IntegrationStatus>,
    /// Metadata.
    pub metadata: Option<serde_json::Value>,
}

/// Data for updating a workspace run.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = workspace_integration_runs)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UpdateWorkspaceIntegrationRun {
    /// Run name.
    pub run_name: Option<String>,
    /// Run status.
    pub run_status: Option<IntegrationStatus>,
    /// Metadata.
    pub metadata: Option<serde_json::Value>,
    /// Started at.
    pub started_at: Option<Option<Timestamp>>,
    /// Completed at.
    pub completed_at: Option<Option<Timestamp>>,
}

impl WorkspaceIntegrationRun {
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
        matches!(self.run_status, IntegrationStatus::Failed)
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

    /// Returns whether the run has custom metadata.
    pub fn has_metadata(&self) -> bool {
        !self.metadata.as_object().is_none_or(|obj| obj.is_empty())
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

    /// Returns whether this is a specific run type.
    pub fn is_type(&self, type_name: &str) -> bool {
        self.run_type.eq_ignore_ascii_case(type_name)
    }
}

impl HasCreatedAt for WorkspaceIntegrationRun {
    fn created_at(&self) -> jiff::Timestamp {
        self.created_at.into()
    }
}

impl HasUpdatedAt for WorkspaceIntegrationRun {
    fn updated_at(&self) -> jiff::Timestamp {
        self.updated_at.into()
    }
}
