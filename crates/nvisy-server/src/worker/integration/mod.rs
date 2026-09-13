//! The connection sync background subsystem: the transfer engine that moves files
//! between an external connection and the first-party blob store, and the worker
//! that schedules and consumes cron-scheduled syncs.
//!
//! The request-side [`ConnectionSyncService`](crate::service::ConnectionSyncService)
//! opens a run and delegates the transfer to the [`TransferEngine`]; the
//! [`ConnectionSyncWorker`] elects a leader per tick, enqueues due connections onto
//! the work-queue, and drains it, running each scheduled sync through the same
//! engine. The direction collaborators (importer, exporter) and the provider-neutral
//! file-source plumbing live here alongside the engine that drives them.

mod connector;
mod engine;
mod export;
mod file_source;
mod import;
mod naming;
mod worker;

pub use engine::TransferEngine;
pub use file_source::SourceEntry;
pub use worker::{ConnectionSyncJob, ConnectionSyncWorker};
