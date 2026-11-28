//! Connection service outputs.

use nvisy_core::Secret;
use nvisy_postgres::model::{
    WorkspaceConnection as WorkspaceConnectionModel, WorkspaceConnectionSchedule,
};
use nvisy_postgres::types::WithAccountRef;

/// A connection with its creator, schedule, and last successful sync time —
/// the shape both a read and a list entry carry.
pub struct FoundConnection {
    /// The connection paired with its creator account reference.
    pub connection: WithAccountRef<WorkspaceConnectionModel>,
    /// The sync schedule, present only for transfer-capable connections.
    pub schedule: Option<WorkspaceConnectionSchedule>,
    /// When the connection last synced successfully, if ever.
    pub last_synced_at: Option<jiff::Timestamp>,
}

/// The result of checking a connection against its backing store.
pub enum ConnectionCheck {
    /// The store is reachable with the stored credentials.
    Reachable,
    /// The store could not be reached or rejected the credentials.
    Unreachable {
        /// A safe reason for the caller, naming no URL, bucket, or other
        /// infrastructure detail.
        reason: &'static str,
    },
}

/// A short-lived provider access token for a browser file picker.
pub struct PickerToken {
    /// The access token.
    pub access_token: Secret<String>,
    /// Expiry as a Unix timestamp, if the provider reported one.
    pub expires_at: Option<i64>,
}
