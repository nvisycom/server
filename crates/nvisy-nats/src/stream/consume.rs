//! The consume loop shared by every work-queue worker.

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use async_nats::jetstream::AckKind;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

use super::core::EventStream;
use super::typed_sub::EventSubscriber;
use crate::{Result, TRACING_TARGET_STREAM};

/// How long one pull waits for a message before checking for cancellation.
const PULL_TIMEOUT: Duration = Duration::from_secs(5);

/// Pause after a failed pull, so a broken connection does not spin the loop.
const PULL_ERROR_BACKOFF: Duration = Duration::from_secs(1);

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
    /// A permit is taken before each pull, so the stream's redelivery lease does
    /// not advance while every slot is busy. On cancellation the loop stops
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
        let mut stream = self.subscribe().await?;
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

            let pulled = tokio::select! {
                () = cancel.cancelled() => break,
                pulled = stream.next_with_timeout(PULL_TIMEOUT) => pulled,
            };
            let message = match pulled {
                Ok(Some(message)) => message,
                Ok(None) => continue,
                Err(err) => {
                    tracing::error!(target: TRACING_TARGET_STREAM, stream = S::NAME, error = %err, "Failed to pull a message");
                    tokio::time::sleep(PULL_ERROR_BACKOFF).await;
                    continue;
                }
            };

            let (payload, message) = message.into_parts();
            let delivered = message.info().map_or(1, |info| info.delivered.max(1));
            let delivery = Delivery {
                attempt: delivered.cast_unsigned(),
                is_final: S::MAX_DELIVER.is_some_and(|max| delivered >= max),
            };
            let handle = handle.clone();
            tasks.spawn(async move {
                let _permit = permit;
                let settle = handle(payload, delivery).await;
                let ack = match settle {
                    Settle::Ack => AckKind::Ack,
                    Settle::Retry => AckKind::Nak(None),
                    Settle::RetryAfter(delay) => AckKind::Nak(Some(delay)),
                };
                if let Err(err) = message.ack_with(ack).await {
                    tracing::error!(target: TRACING_TARGET_STREAM, stream = S::NAME, error = %err, ?settle, "Failed to settle a message");
                }
            });
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
