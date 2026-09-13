//! Background execution layer.
//!
//! Each subsystem's background executors live here, alongside the [`Worker`]
//! supervision contract they share. The request-side handles that enqueue work
//! for them stay in [`service`](crate::service) and are reached per request; the
//! executors run off the request thread until the shared cancellation token
//! fires.

mod coordinator;
mod set;

pub mod assistant;
pub mod detection;
pub mod integration;

pub use coordinator::Coordinator;
pub use set::{Worker, WorkerSet};
