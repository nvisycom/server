//! Background execution layer.
//!
//! Each subsystem's background executors live here, alongside the [`Worker`]
//! supervision contract they share. The request-side handles that enqueue work
//! for them stay in [`service`] and are reached per request; the executors run
//! off the request thread until the shared cancellation token fires.
//!
//! [`service`]: crate::service

mod coordinator;
mod set;

pub mod assistant;
pub mod detection;
pub mod event;
pub mod integration;
pub mod reaper;
pub mod webhook;

pub use coordinator::Coordinator;
use nvisy_nats::NatsClient;
pub use reaper::BlobReaper;
pub use set::{Worker, WorkerSet};

use crate::Result;

/// Reconciles every `JetStream` stream the workers use, creating each if absent and
/// updating its config to match.
///
/// Called once at startup so stream reconciliation happens in one place rather
/// than on every publisher or subscriber construction. Publishers and subscribers
/// built afterwards are then cheap handles that assume their stream already exists.
///
/// # Errors
///
/// A NATS error if reconciling any of the streams (create or update) fails.
pub async fn ensure_streams(nats: &NatsClient) -> Result<()> {
    nats.ensure_stream::<webhook::WebhookStream>().await?;
    nats.ensure_stream::<detection::DetectionStream>().await?;
    nats.ensure_stream::<assistant::AssistantStream>().await?;
    nats.ensure_stream::<integration::SyncStream>().await?;
    Ok(())
}
