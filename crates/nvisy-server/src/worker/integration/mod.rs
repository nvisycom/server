//! The connection sync background subsystem: the transfer engine that moves files
//! between an external connection and the first-party blob store, and the worker
//! that schedules and consumes cron-scheduled syncs.
//!
//! The request-side [`ConnectionSyncService`] opens a run and delegates the
//! transfer to the [`TransferEngine`]; the [`ConnectionSyncWorker`] elects a
//! leader per tick, enqueues due connections onto the work-queue, and drains it,
//! running each scheduled sync through the same engine. The direction
//! collaborators (importer, exporter) and the provider-neutral file-source
//! plumbing live here alongside the engine that drives them.
//!
//! [`ConnectionSyncService`]: crate::service::ConnectionSyncService

mod connector;
mod engine;
mod export;
mod file_source;
mod import;
mod naming;
mod worker;

use std::future::Future;

pub use engine::TransferEngine;
pub use file_source::SourceEntry;
use futures::stream::{self, StreamExt};
pub(crate) use worker::SchedulerLocksBucket;
pub use worker::{ConnectionSyncJob, ConnectionSyncWorker, SyncStream};

/// Runs `transfer` over `items`, at most `concurrency` at once, and counts the
/// ones that succeeded. `transfer` logs its own failure, so one bad file is
/// skipped rather than aborting the batch.
async fn count_transferred<T, F, Fut>(items: Vec<T>, concurrency: usize, transfer: F) -> u64
where
    F: Fn(T) -> Fut,
    Fut: Future<Output = bool>,
{
    let succeeded = stream::iter(items)
        .map(transfer)
        .buffer_unordered(concurrency)
        .filter(|ok| std::future::ready(*ok))
        .count()
        .await;
    u64::try_from(succeeded).unwrap_or(u64::MAX)
}
