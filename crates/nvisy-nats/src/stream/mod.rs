//! JetStream streams for real-time updates and distributed job processing.
//!
//! This module provides type-safe streaming capabilities: generic event
//! publishing and subscribing over a stream configured via [`EventStream`].

mod broadcast_stream;
mod event_stream;
mod generic_stream;
mod typed_stream;
mod typed_stream_pub;
mod typed_stream_sub;

pub use broadcast_stream::BroadcastStream;
pub use event_stream::{ConnectionSyncStream, DetectionStream, EventStream, WebhookStream};
pub use generic_stream::{EventPublisher, EventSubscriber};
pub use typed_stream::{TypedBatchStream, TypedMessage, TypedMessageStream};
