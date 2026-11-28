//! The request-side handle for the assistant subsystem.

use crate::worker::Coordinator;

/// Wakes the assistant-job outbox drainer after a reply job is committed.
///
/// Cheaply cloneable (the [`Coordinator`] is `Arc`-backed).
#[derive(Clone)]
#[must_use = "service does nothing unless you wake the drainer with it"]
pub struct AssistantQueue {
    coordinator: Coordinator,
}

impl AssistantQueue {
    /// Creates a new [`AssistantQueue`].
    pub fn new(coordinator: Coordinator) -> Self {
        Self { coordinator }
    }

    /// Wakes the assistant-job outbox drainer so a just-committed job is drained
    /// immediately instead of waiting for the drainer's next timer tick. Call after
    /// the transaction that inserted the job row has committed. Best-effort and
    /// coalescing: the drainer's timer still covers a missed wake (a crash between
    /// commit and this call, or a job that landed on another instance).
    pub fn wake_drainer(&self) {
        self.coordinator.wake();
    }
}
