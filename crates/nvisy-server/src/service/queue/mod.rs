//! Request-time handles that enqueue background work and broadcast/subscribe to
//! status.
//!
//! Each handle is the request-side counterpart to a background executor in
//! [`worker`](crate::worker): a handler (or domain service) enqueues a job
//! transactionally, wakes the subsystem's drainer, and — for detections — relays
//! status to watchers. The executors that consume the jobs live in
//! [`worker::assistant`](crate::worker::assistant) and
//! [`worker::detection`](crate::worker::detection).

mod assistant;
mod detection;

pub use assistant::AssistantQueue;
pub use detection::DetectionQueue;
