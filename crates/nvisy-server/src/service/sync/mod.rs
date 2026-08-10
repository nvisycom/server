//! Connection sync: importing objects from and exporting them to external
//! storage connections, plus the scheduler that drives periodic syncs.
//!
//! Built on top of [`ObjectService`](crate::service::ObjectService) (bare
//! object-store access): [`ConnectionSyncService`] orchestrates the transfers,
//! [`ConnectionSyncWorker`] schedules them, and [`StandardCronSchedule`] decides
//! when a cron-scheduled connection is due.

mod bridge;
mod cron_schedule;
mod service;
mod worker;

pub use cron_schedule::StandardCronSchedule;
pub use service::ConnectionSyncService;
pub use worker::{ConnectionSyncJob, ConnectionSyncWorker};

/// Default number of objects imported concurrently per sync. Kept well below the
/// default Postgres pool size (10) since each in-flight import briefly checks out
/// a pooled connection for its bookkeeping, and the pool is shared with the HTTP
/// handlers and other workers.
pub const DEFAULT_IMPORT_CONCURRENCY: usize = 4;

/// Tunables for connection sync.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "cli", derive(clap::Args))]
pub struct SyncConfig {
    /// Maximum objects imported concurrently within a single sync. Bounds the
    /// in-flight fetch/decrypt/store pipelines against one connection.
    #[cfg_attr(
        feature = "cli",
        arg(
            long = "sync-import-concurrency",
            env = "SYNC_IMPORT_CONCURRENCY",
            default_value_t = DEFAULT_IMPORT_CONCURRENCY,
        )
    )]
    pub import_concurrency: usize,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            import_concurrency: DEFAULT_IMPORT_CONCURRENCY,
        }
    }
}
