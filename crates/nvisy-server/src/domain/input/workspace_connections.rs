//! Connection service inputs.

use nvisy_postgres::types::{SyncDeletionPolicy, SyncMode};

use crate::service::ConnectionConfig;

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
