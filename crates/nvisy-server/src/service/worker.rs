//! Background worker supervision.
//!
//! Every background worker shares the same lifecycle — `run` until the shared
//! [`CancellationToken`] fires, logging its own start/stop/failure. [`Worker`]
//! captures that shape and [`WorkerSet`] owns the spawn/cancel/join bookkeeping
//! once, so the binary only has to enumerate which workers to run.

use std::future::Future;

use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

/// Tracing target for worker supervision.
const TRACING_TARGET: &str = "nvisy_server::service::worker";

/// A background worker: runs until cancelled, logging its own lifecycle.
///
/// Implementors run a loop that stops when the passed [`CancellationToken`] is
/// cancelled and return once it has drained. Each worker logs its own outcome,
/// so the supervisor discards the returned value; the associated [`Output`] type
/// only lets workers keep their own `Result` alias. The `name` labels the task
/// in supervision logs (e.g. on a panic during join).
///
/// [`Output`]: Worker::Output
pub trait Worker: Send + 'static {
    /// What [`run`](Worker::run) resolves to. Ignored by the supervisor; present
    /// so each worker can return its own `Result` type unchanged.
    type Output: Send;

    /// A short, stable label for this worker, used in supervision logs.
    fn name(&self) -> &'static str;

    /// Runs the worker until `cancel` fires.
    fn run(&self, cancel: CancellationToken) -> impl Future<Output = Self::Output> + Send;
}

/// Spawns and supervises a group of [`Worker`]s under one shared cancellation
/// token.
///
/// [`spawn`](WorkerSet::spawn) each worker, run the foreground work, then
/// [`shutdown`](WorkerSet::shutdown) to cancel and join them all — a panic in
/// any task is logged, never propagated, so shutdown always completes.
#[must_use = "spawned workers are not awaited until you call shutdown"]
pub struct WorkerSet {
    cancel: CancellationToken,
    handles: Vec<(&'static str, JoinHandle<()>)>,
}

impl WorkerSet {
    /// Creates an empty set with a fresh cancellation token.
    pub fn new() -> Self {
        Self {
            cancel: CancellationToken::new(),
            handles: Vec::new(),
        }
    }

    /// Spawns `worker` on the runtime, tied to this set's cancellation token.
    pub fn spawn<W: Worker>(&mut self, worker: W) {
        let name = worker.name();
        let cancel = self.cancel.clone();
        let handle = tokio::spawn(async move {
            let _ = worker.run(cancel).await;
        });
        self.handles.push((name, handle));
    }

    /// Signals every worker to stop and waits for each to finish, logging (never
    /// propagating) a task that panicked.
    pub async fn shutdown(self) {
        self.cancel.cancel();
        for (name, handle) in self.handles {
            if let Err(err) = handle.await {
                tracing::error!(
                    target: TRACING_TARGET,
                    worker = name,
                    error = %err,
                    "Worker task panicked",
                );
            }
        }
    }
}

impl Default for WorkerSet {
    fn default() -> Self {
        Self::new()
    }
}
