//! Connection service outputs.

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
