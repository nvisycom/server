//! Connection capability-category enumeration.

use super::db_enum;

db_enum! {
    /// The capability category of a transfer connection.
    ///
    /// Corresponds to the `CONNECTION_TYPE` PostgreSQL enum. A stable, closed set:
    /// the concrete provider (the `provider` column, e.g. `s3`) stays open and
    /// extensible, while its capability is one of these types. Both categories are
    /// transfer-capable — an object store is enumerable and syncs on a timer; a
    /// file service transfers on demand. Inference services are a separate
    /// resource (`workspace_providers`), not a connection.
    pub enum ConnectionType = "crate::schema::sql_types::ConnectionType" {
        /// External object storage (s3, azure, gcs, ...).
        ObjectStore = "object_store",
        /// External file service (google_drive, dropbox, ...).
        FileService = "file_service",
    }
}

impl ConnectionType {
    /// Whether a connection of this category can be given a scheduled-sync
    /// configuration (a cron that runs the transfer on a timer).
    ///
    /// Scheduling requires the source to be enumerable on a timer, which today is
    /// object stores only: a file service transfers on demand (picker import,
    /// per-file export), not on a schedule. Keyed on the category so a caller
    /// holding only the stored `connection_type` — without decrypting the config —
    /// can apply the same rule. This is the single seam to widen if file-service
    /// automation is added later; no schema change is required to do so.
    #[must_use]
    pub fn supports_schedule(self) -> bool {
        matches!(self, Self::ObjectStore)
    }
}
