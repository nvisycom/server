//! Connection service inputs.

use nvisy_postgres::model::NewWorkspaceConnectionSchedule;
use nvisy_postgres::types::{SyncDeletionPolicy, SyncMode};
use uuid::Uuid;

use crate::response::{ErrorKind, Result};
use crate::service::{ConnectionConfig, StandardCronSchedule};

/// Scheduled-sync configuration for a schedulable connection.
#[derive(Clone)]
pub struct SyncScheduleInput {
    /// Whether the connection imports data in or exports data out.
    pub sync_mode: SyncMode,
    /// Cron expression for scheduled imports; `None` for manual-only.
    pub schedule_cron: Option<String>,
    /// How an import reconciles files whose source object was deleted.
    pub deletion_policy: SyncDeletionPolicy,
}

impl SyncScheduleInput {
    /// Checks the schedule is usable: the connection's provider supports
    /// scheduled sync, and the cron expression (if any) is valid.
    ///
    /// # Errors
    ///
    /// `BadRequest` if scheduling is unsupported or the cron is invalid.
    pub fn validate(&self, supports_schedule: bool) -> Result<()> {
        if !supports_schedule {
            return Err(
                ErrorKind::BadRequest.with_message("This provider does not support scheduled sync")
            );
        }
        if let Some(cron) = &self.schedule_cron
            && !StandardCronSchedule.is_valid(cron)
        {
            return Err(ErrorKind::BadRequest.with_message("Invalid cron expression"));
        }
        Ok(())
    }

    /// The schedule row for `connection_id`.
    pub fn into_model(self, connection_id: Uuid) -> NewWorkspaceConnectionSchedule {
        NewWorkspaceConnectionSchedule {
            connection_id,
            sync_mode: Some(self.sync_mode),
            schedule_cron: self.schedule_cron,
            deletion_policy: Some(self.deletion_policy),
        }
    }
}

/// Input for creating a connection.
pub struct CreateConnectionInput {
    /// Human-readable connection display name.
    pub display_name: String,
    /// Whether the connection is enabled; `None` defaults to active.
    pub is_active: Option<bool>,
    /// Typed provider configuration.
    pub config: ConnectionConfig,
    /// Scheduled-sync configuration; accepted only for schedulable providers.
    pub sync: Option<SyncScheduleInput>,
}

/// Input for updating a connection. A present `config` fully replaces the stored
/// one; omitted fields are left unchanged.
pub struct UpdateConnectionInput {
    /// New display name.
    pub display_name: Option<String>,
    /// New active state.
    pub is_active: Option<bool>,
    /// Replacement configuration.
    pub config: Option<ConnectionConfig>,
    /// Scheduled-sync configuration.
    pub sync: Option<SyncScheduleInput>,
}
