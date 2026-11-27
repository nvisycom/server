//! Request-time handles that wake background work and broadcast/subscribe to
//! status.
//!
//! Each handle is the request-side counterpart to a background executor in
//! [`worker`]: a domain service writes a job's outbox row transactionally, then
//! wakes the subsystem's drainer through its handle, and — for detections — the
//! handle relays status to watchers. The executors that consume the jobs live in [`worker::assistant`]
//! and [`worker::detection`].
//!
//! [`worker`]: crate::worker
//! [`worker::assistant`]: crate::worker::assistant
//! [`worker::detection`]: crate::worker::detection

mod assistant;
mod detection;

pub use assistant::AssistantQueue;
pub use detection::DetectionQueue;
