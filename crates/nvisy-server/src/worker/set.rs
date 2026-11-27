//! Background worker supervision.
//!
//! Every background worker shares the same lifecycle — `run` until the shared
//! [`CancellationToken`] fires. [`Worker`] captures that shape, and [`WorkerSet`]
//! owns the spawn/cancel/join bookkeeping and the lifecycle logging once, so a
//! worker only implements its loop and the binary only enumerates which workers
//! to run.

use std::fmt::Display;
use std::future::Future;
use std::time::Duration;

use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use super::Coordinator;

/// Tracing target for worker supervision.
const TRACING_TARGET: &str = "nvisy_server::worker";

/// A background worker: runs until cancelled.
///
/// Implementors run a loop that stops when the passed [`CancellationToken`] is
/// cancelled and return once it has drained. The supervisor logs the start, the
/// stop, and a returned error under [`NAME`](Worker::NAME).
pub trait Worker: Send + Sync + 'static {
    /// A short, stable label for this worker, used in supervision logs.
    const NAME: &'static str;

    /// What a failed run reports.
    type Error: Display + Send;

    /// Runs the worker until `cancel` fires.
    fn run(
        &self,
        cancel: CancellationToken,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send;
}

/// Spawns and supervises a group of [`Worker`]s under one shared cancellation
/// token.
///
/// [`spawn`](WorkerSet::spawn) each worker, run the foreground work, then
/// [`shutdown_with_timeout`](WorkerSet::shutdown_with_timeout) to cancel and join
/// them all — a panic in any task is logged, never propagated, so shutdown always
/// completes.
#[must_use = "spawned workers are not awaited until you call shutdown"]
pub struct WorkerSet {
    cancel: CancellationToken,
    handles: Vec<(&'static str, JoinHandle<()>)>,
}

impl WorkerSet {
    /// Creates an empty set driven by an existing cancellation token, so workers
    /// stop when that shared token is cancelled (e.g. the app-wide shutdown
    /// signal).
    pub fn with_token(cancel: CancellationToken) -> Self {
        Self {
            cancel,
            handles: Vec::new(),
        }
    }

    /// Spawns `worker` on the runtime, tied to this set's cancellation token, and
    /// logs its start, stop, or failure.
    pub fn spawn<W: Worker>(&mut self, worker: W) {
        let cancel = self.cancel.clone();
        let handle = tokio::spawn(async move {
            tracing::info!(target: TRACING_TARGET, worker = W::NAME, "Worker started");
            match worker.run(cancel).await {
                Ok(()) => {
                    tracing::info!(target: TRACING_TARGET, worker = W::NAME, "Worker stopped");
                }
                Err(err) => {
                    tracing::error!(target: TRACING_TARGET, worker = W::NAME, error = %err, "Worker failed");
                }
            }
        });
        self.handles.push((W::NAME, handle));
    }

    /// Signals every worker to stop and waits for each to finish, bounded by
    /// `timeout`: a worker that does not observe its cancellation token within the
    /// deadline is abandoned (its task detached) rather than blocking process
    /// exit. A task that panicked is logged, never propagated.
    ///
    /// Cancellation is signalled once up front, so all workers wind down in
    /// parallel and the whole join shares the single `timeout` budget.
    pub async fn shutdown_with_timeout(self, timeout: Duration) {
        self.cancel.cancel();
        let join_all = async {
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
        };
        if tokio::time::timeout(timeout, join_all).await.is_err() {
            tracing::warn!(
                target: TRACING_TARGET,
                timeout_secs = timeout.as_secs(),
                "Workers did not stop within the shutdown timeout; abandoning remaining tasks",
            );
        }
    }
}

/// Runs `pass` every `interval` (the first immediately) and, when `wake` is
/// given, whenever it is woken, until `cancel` fires.
///
/// A wake stored while a pass runs coalesces into one following pass. The timer
/// stays the fallback (and the cross-instance safety net) for a missed wake.
pub(crate) async fn every<F, Fut>(
    interval: Duration,
    cancel: &CancellationToken,
    wake: Option<&Coordinator>,
    mut pass: F,
) where
    F: FnMut() -> Fut,
    Fut: Future<Output = ()>,
{
    let mut ticker = tokio::time::interval(interval);
    loop {
        tokio::select! {
            () = cancel.cancelled() => break,
            _ = ticker.tick() => pass().await,
            () = async {
                match wake {
                    Some(wake) => wake.notified().await,
                    None => std::future::pending().await,
                }
            } => pass().await,
        }
    }
}
