//! The integration subsystem: external storage and file-service connections and
//! the transfers across them.
//!
//! A connection is a typed [`ConnectionConfig`] (an object store, a cloud file
//! service, or an inference provider) that the server reaches with the tenant's
//! credentials. [`IntegrationConfig`] holds the deployment knobs (endpoint policy,
//! import concurrency), and [`FileConnectorsConfig`] the cloud file-service OAuth
//! apps.
//!
//! Built on top of [`ExternalObjectStore`] (bare object-store access):
//! [`ConnectionSyncService`] is the request-side handle that opens runs and
//! delegates transfers to the [`TransferEngine`], which the
//! [`ConnectionSyncWorker`] schedules; [`StandardCronSchedule`] decides when a
//! cron-scheduled connection is due.
//!
//! [`ExternalObjectStore`]: crate::service::ExternalObjectStore
//! [`TransferEngine`]: crate::worker::integration::TransferEngine
//! [`ConnectionSyncWorker`]: crate::worker::integration::ConnectionSyncWorker

mod connection_config;
mod connectors;
mod cron_schedule;
mod deployment;
mod persist_oauth;
mod provider_config;
mod service;

pub use connection_config::ConnectionConfig;
pub use connectors::{FileConnectorsConfig, FileServiceRedirect};
pub use cron_schedule::StandardCronSchedule;
pub use deployment::IntegrationConfig;
pub use persist_oauth::persist_refreshed_tokens;
pub use provider_config::ProviderConfig;
pub use service::{ConnectionSyncService, TransferKind, TransferRequest};
