//! The consume loop shared by every work-queue worker.

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use async_nats::jetstream::{AckKind, Message};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tokio::time::MissedTickBehavior;
use tokio_util::sync::CancellationToken;
use tokio_util::task::AbortOnDropHandle;

use super::core::EventStream;
use super::typed_sub::EventSubscriber;
use crate::{Result, TRACING_TARGET_STREAM};

/// How long one pull waits for a message before checking for cancellation.
const PULL_TIMEOUT: Duration = Duration::from_secs(5);

/// Pause after a failed pull, so a broken connection does not spin the loop.
const PULL_ERROR_BACKOFF: Duration = Duration::from_secs(1);

/// The ack lease the server applies when a stream sets none.
const DEFAULT_ACK_WAIT: Duration = Duration::from_secs(30);

/// Extends a message's ack lease every third of the lease, so two missed beats
/// still leave it alive, until its task is aborted. Each progress ack is bounded
/// by the period, so a stalled send skips a beat rather than piling up.
async fn extend_lease<S: EventStream>(message: Message) {
    let period = S::ACK_WAIT.unwrap_or(DEFAULT_ACK_WAIT) / 3;
    let mut beat = tokio::time::interval(period);
    beat.set_missed_tick_behavior(MissedTickBehavior::Delay);
    beat.tick().await;
    loop {
        beat.tick().await;
        match tokio::time::timeout(period, message.ack_with(AckKind::Progress)).await {
            Ok(Ok(())) => {}
            Ok(Err(err)) => {
                tracing::warn!(target: TRACING_TARGET_STREAM, stream = S::NAME, error = %err, "Failed to extend a message's ack lease");
            }
            Err(_) => {
                tracing::warn!(target: TRACING_TARGET_STREAM, stream = S::NAME, "Timed out extending a message's ack lease");
            }
        }
    }
}

/// How a handled message is settled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Settle {
    /// Done (or safe to drop): acknowledge, so it is not redelivered.
    Ack,
    /// Transient failure: redeliver now.
    Retry,
    /// Transient failure: redeliver after the delay.
    RetryAfter(Duration),
}

/// Which delivery of a message this is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Delivery {
    /// 1-based delivery count.
    pub attempt: u64,
    /// Whether the stream will not redeliver after this attempt (its
    /// `MAX_DELIVER` is reached), so a retry would drop the message.
    pub is_final: bool,
}

impl<S: EventStream> EventSubscriber<S> {
    /// Consumes the stream until `cancel` fires, running `handle` on each
    /// message with at most `concurrency` in flight and settling it as `handle`
    /// returns.
    ///
    /// A permit is taken before each pull, and each pull requests exactly one
    /// message, so a message is delivered only to a free slot: its ack lease
    /// starts when its handler does, and its delivery count is the server's.
    /// While a handler runs, the lease is extended on a heartbeat, so `ACK_WAIT`
    /// bounds how long a stalled or crashed handler holds a message, not how
    /// long a job may run. On cancellation the loop stops
    /// pulling and waits for in-flight handlers to settle their messages; the
    /// caller's shutdown timeout bounds that wait, and a message whose handler is
    /// aborted is redelivered.
    ///
    /// # Errors
    ///
    /// - `Operation` if the consumer cannot be opened. Errors on individual pulls
    ///   and settlements are logged and the loop continues.
    pub async fn consume<F, Fut>(
        &self,
        concurrency: usize,
        cancel: &CancellationToken,
        handle: F,
    ) -> Result<()>
    where
        F: Fn(S::Message, Delivery) -> Fut + Clone + Send + 'static,
        Fut: Future<Output = Settle> + Send + 'static,
    {
        let stream = self.subscribe().await?;
        let slots = Arc::new(Semaphore::new(concurrency.max(1)));
        let mut tasks: JoinSet<()> = JoinSet::new();

        loop {
            let permit = tokio::select! {
                () = cancel.cancelled() => break,
                // Reap finished tasks so the set stays bounded by what is in flight.
                Some(_) = tasks.join_next() => continue,
                permit = slots.clone().acquire_owned() => match permit {
                    Ok(permit) => permit,
                    Err(_) => break,
                },
            };

            // The pull is not raced against cancellation: dropping it mid-flight
            // could strand a message the server already sent to its inbox until
            // the ack lease ran out. It ends within `PULL_TIMEOUT`.
            let (payload, message) = match stream.next(PULL_TIMEOUT).await {
                Ok(Some(pulled)) => pulled,
                Ok(None) => continue,
                Err(err) => {
                    tracing::error!(target: TRACING_TARGET_STREAM, stream = S::NAME, error = %err, "Failed to pull a message");
                    tokio::time::sleep(PULL_ERROR_BACKOFF).await;
                    continue;
                }
            };
            let delivered = message.info().map_or(1, |info| info.delivered.max(1));
            let delivery = Delivery {
                attempt: delivered.cast_unsigned(),
                is_final: S::MAX_DELIVER.is_some_and(|max| delivered >= max),
            };

            // A message pulled after cancellation is handed back, unless this is
            // its final delivery: a NAK then would drop it, so it is handled like
            // any in-flight message and the drain below waits for it.
            let stopping = cancel.is_cancelled();
            if stopping && !delivery.is_final {
                if let Err(err) = message.ack_with(AckKind::Nak(None)).await {
                    tracing::warn!(target: TRACING_TARGET_STREAM, stream = S::NAME, error = %err, "Failed to return a message pulled at shutdown");
                }
                break;
            }

            let handle = handle.clone();
            tasks.spawn(async move {
                let _permit = permit;
                // Extend the ack lease while the handler runs, so a long but
                // healthy job is not redelivered to another instance mid-run. The
                // heartbeat runs on its own task, so neither a slow progress ack
                // nor a blocking stretch in the handler stalls the other. The
                // handle aborts it on drop, so a handler that panics or is
                // aborted stops renewing the lease and the message is redelivered.
                let heartbeat =
                    AbortOnDropHandle::new(tokio::spawn(extend_lease::<S>(message.clone())));
                let settle = handle(payload, delivery).await;
                drop(heartbeat);

                let ack = match settle {
                    Settle::Ack => AckKind::Ack,
                    Settle::Retry => AckKind::Nak(None),
                    Settle::RetryAfter(delay) => AckKind::Nak(Some(delay)),
                };
                if let Err(err) = message.ack_with(ack).await {
                    tracing::error!(target: TRACING_TARGET_STREAM, stream = S::NAME, error = %err, ?settle, "Failed to settle a message");
                }
            });
            if stopping {
                break;
            }
        }

        if !tasks.is_empty() {
            tracing::info!(
                target: TRACING_TARGET_STREAM,
                stream = S::NAME,
                in_flight = tasks.len(),
                "Draining in-flight messages before stopping",
            );
            while tasks.join_next().await.is_some() {}
        }
        Ok(())
    }
}
