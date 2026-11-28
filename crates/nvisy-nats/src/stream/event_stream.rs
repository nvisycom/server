//! The concrete event-stream markers, each pinning an [`EventStream`]'s config.

use std::marker::PhantomData;
use std::time::Duration;

use serde::Serialize;
use serde::de::DeserializeOwned;

use super::core::EventStream;

/// Stream for webhook delivery.
///
/// Messages expire after 1 day. NATS redelivery is the primary retry layer for
/// self-hosted delivery: the worker redelivers a failed attempt after a linear
/// backoff, up to `MAX_DELIVER` attempts. `ACK_WAIT` exceeds the per-attempt
/// delivery timeout, so an attempt still in flight is not redelivered.
///
/// The payload is left to the consumer (the job type it enqueues), so the stream
/// is generic over `M`; a consumer pins it with a type alias, e.g.
/// `type WebhookStream = nvisy_nats::stream::WebhookStream<WebhookJob>`.
pub enum WebhookStream<M> {
    #[doc(hidden)]
    Never(PhantomData<fn() -> M>),
}

impl<M> EventStream for WebhookStream<M>
where
    M: Serialize + DeserializeOwned + Send + Sync + 'static,
{
    type Message = M;

    const ACK_WAIT: Option<Duration> = Some(Duration::from_secs(90));
    const CONSUMER_NAME: &'static str = "webhook-worker";
    const DESCRIPTION: &'static str = "Webhook delivery jobs";
    const MAX_AGE: Option<Duration> = Some(Duration::from_hours(24));
    const MAX_DELIVER: Option<i64> = Some(3);
    const NAME: &'static str = "WEBHOOKS";
    const SUBJECT: &'static str = "webhooks";
}

/// Work queue for connection sync jobs.
///
/// Scheduled syncs are enqueued here. A single shared durable consumer delivers
/// each job to one instance at a time (at-least-once); consumers make jobs
/// idempotent so a redelivery is safe. Messages expire after 3 hours, long
/// enough for every attempt, so a backlog cannot pile up.
///
/// Generic over its payload `M`; a consumer pins it with a type alias, e.g.
/// `type ConnectionSyncStream = nvisy_nats::stream::ConnectionSyncStream<SyncJob>`.
pub enum ConnectionSyncStream<M> {
    #[doc(hidden)]
    Never(PhantomData<fn() -> M>),
}

impl<M> EventStream for ConnectionSyncStream<M>
where
    M: Serialize + DeserializeOwned + Send + Sync + 'static,
{
    type Message = M;

    // The consumer extends the lease while a transfer runs; the lease still
    // exceeds the 30-minute transfer timeout, as a margin for a handler too busy
    // to send its heartbeat.
    const ACK_WAIT: Option<Duration> = Some(Duration::from_mins(35));
    const CONSUMER_NAME: &'static str = "connection-sync-worker";
    const DESCRIPTION: &'static str = "Connection sync jobs";
    // A retry redelivers the same message, so its age runs from the first
    // publish: cover three 30-minute transfers, their backoffs, and time queued
    // behind other syncs.
    const MAX_AGE: Option<Duration> = Some(Duration::from_hours(3));
    // A failed scheduled sync is redelivered with a backoff; three attempts in
    // all, then it is left failed.
    const MAX_DELIVER: Option<i64> = Some(3);
    const NAME: &'static str = "CONNECTION_SYNCS";
    const SUBJECT: &'static str = "connection.sync.jobs";
}

/// Work queue for detection jobs.
///
/// A detection is created synchronously, then its analyze step is enqueued here
/// and handled by a background worker. A single shared durable consumer delivers
/// each job to one instance at a time (at-least-once); the worker is idempotent
/// on the detection so a redelivery is safe. Messages expire after 3 hours so a
/// backlog cannot pile up.
///
/// Generic over its payload `M`; a consumer pins it with a type alias, e.g.
/// `type DetectionStream = nvisy_nats::stream::DetectionStream<DetectionJob>`.
pub enum DetectionStream<M> {
    #[doc(hidden)]
    Never(PhantomData<fn() -> M>),
}

impl<M> EventStream for DetectionStream<M>
where
    M: Serialize + DeserializeOwned + Send + Sync + 'static,
{
    type Message = M;

    // The consumer extends the lease while an analysis runs; the lease still
    // exceeds the longest expected analysis (LLM/OCR), as a margin for a handler
    // too busy to send its heartbeat.
    const ACK_WAIT: Option<Duration> = Some(Duration::from_mins(15));
    const CONSUMER_NAME: &'static str = "detection-worker";
    const DESCRIPTION: &'static str = "Detection jobs";
    // A retry after a failed attempt waits out the worker's 30-minute claim
    // lease and redelivers the same message, so its age covers several leases.
    const MAX_AGE: Option<Duration> = Some(Duration::from_hours(3));
    const NAME: &'static str = "DETECTIONS";
    const SUBJECT: &'static str = "detection.jobs";
}

/// Work queue for assistant-reply jobs.
///
/// When a user addresses the assistant in a comment thread, a job is enqueued
/// here; a single shared durable consumer delivers it to one worker at a time
/// (at-least-once). The worker is idempotent on the triggering comment so a
/// redelivery does not post a second reply. Inference can be slow, so `ACK_WAIT`
/// exceeds the longest expected turn; `MAX_DELIVER` bounds retries so a job that
/// can never succeed (e.g. a workspace with no model provider) is not redelivered
/// forever. Messages expire after 1 hour so a backlog cannot pile up.
///
/// Generic over its payload `M`; a consumer pins it with a type alias, e.g.
/// `type AssistantStream = nvisy_nats::stream::AssistantStream<AssistantJob>`.
pub enum AssistantStream<M> {
    #[doc(hidden)]
    Never(PhantomData<fn() -> M>),
}

impl<M> EventStream for AssistantStream<M>
where
    M: Serialize + DeserializeOwned + Send + Sync + 'static,
{
    type Message = M;

    const ACK_WAIT: Option<Duration> = Some(Duration::from_mins(5));
    const CONSUMER_NAME: &'static str = "assistant-worker";
    const DESCRIPTION: &'static str = "Assistant reply jobs";
    const MAX_AGE: Option<Duration> = Some(Duration::from_hours(1));
    const MAX_DELIVER: Option<i64> = Some(5);
    const NAME: &'static str = "ASSISTANT";
    const SUBJECT: &'static str = "assistant.replies";
}

#[cfg(test)]
mod tests {
    use super::*;

    // Consts are payload-independent; any payload pins the generic for the test.
    type Stream = WebhookStream<()>;

    #[test]
    fn test_webhook_stream() {
        assert_eq!(Stream::NAME, "WEBHOOKS");
        assert_eq!(Stream::SUBJECT, "webhooks");
        assert_eq!(Stream::MAX_AGE, Some(Duration::from_hours(24)));
        assert_eq!(Stream::CONSUMER_NAME, "webhook-worker");
    }
}
