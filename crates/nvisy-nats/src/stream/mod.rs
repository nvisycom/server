//! JetStream streams for real-time updates and distributed job processing.
//!
//! This module provides type-safe streaming capabilities: generic event
//! publishing and subscribing over a stream configured via [`EventStream`].

mod event_pub;
mod event_stream;
mod event_sub;
mod stream_pub;
mod stream_sub;

pub use event_pub::EventPublisher;
pub use event_stream::{EventStream, WebhookStream};
pub use event_sub::EventSubscriber;
pub use stream_pub::StreamPublisher;
pub use stream_sub::{StreamSubscriber, TypedBatchStream, TypedMessage, TypedMessageStream};
