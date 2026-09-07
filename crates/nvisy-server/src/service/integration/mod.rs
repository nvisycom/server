//! The integration subsystem: external storage and file-service connections and
//! the transfers across them.
//!
//! A connection is a typed [`ConnectionConfig`] (an object store, a cloud file
//! service, or an inference provider) that the server reaches with the tenant's
//! credentials. [`IntegrationConfig`] holds the deployment knobs (endpoint policy,
//! import concurrency), and [`FileConnectorsConfig`] the cloud file-service OAuth
//! apps.
//!
//! Built on top of [`ExternalObjectStore`](crate::service::ExternalObjectStore)
//! (bare object-store access): [`ConnectionSyncService`] orchestrates the
//! transfers, [`ConnectionSyncWorker`] schedules them, and [`StandardCronSchedule`]
//! decides when a cron-scheduled connection is due.

mod connection_config;
mod connector;
mod connectors;
mod cron_schedule;
mod deployment;
mod export;
mod file_source;
mod import;
mod naming;
mod persist_oauth;
mod service;
mod worker;

pub use connection_config::ConnectionConfig;
pub use connectors::{FileConnectorsConfig, FileServiceRedirect};
pub use cron_schedule::StandardCronSchedule;
pub use deployment::IntegrationConfig;
pub use file_source::SourceEntry;
pub use persist_oauth::persist_refreshed_tokens;
pub use service::{ConnectionSyncService, TransferKind, TransferRequest};
pub use worker::{ConnectionSyncJob, ConnectionSyncWorker};
