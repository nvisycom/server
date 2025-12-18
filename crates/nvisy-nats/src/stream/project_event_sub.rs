//! Project event stream subscriber for real-time WebSocket communication.

use async_nats::jetstream::Context;
use derive_more::{Deref, DerefMut};
use uuid::Uuid;

use super::project_event::ProjectEvent;
use super::subscriber::{StreamSubscriber, TypedBatchStream, TypedMessage, TypedMessageStream};
use crate::Result;

/// Project event subscriber for receiving WebSocket messages.
#[derive(Debug, Clone, Deref, DerefMut)]
pub struct ProjectEventSubscriber {
    #[deref]
    #[deref_mut]
    subscriber: StreamSubscriber<ProjectEvent>,
}

impl ProjectEventSubscriber {
    /// Create a new project event subscriber.
    ///
    /// # Arguments
    ///
    /// * `jetstream` - JetStream context
    /// * `consumer_name` - Unique name for this consumer (e.g., "server-instance-1")
    pub async fn new(jetstream: &Context, consumer_name: &str) -> Result<Self> {
        let subscriber = StreamSubscriber::new(jetstream, "PROJECT_EVENTS", consumer_name).await?;
        Ok(Self { subscriber })
    }

    /// Create a subscriber filtered to a specific project.
    ///
    /// Only receives events for the specified project ID.
    pub async fn new_for_project(
        jetstream: &Context,
        consumer_name: &str,
        project_id: Uuid,
    ) -> Result<Self> {
        let subscriber = StreamSubscriber::new(jetstream, "PROJECT_EVENTS", consumer_name)
            .await?
            .with_filter_subject(format!("PROJECT_EVENTS.{}", project_id));
        Ok(Self { subscriber })
    }
}

/// Type alias for project event batch stream.
pub type ProjectEventBatchStream = TypedBatchStream<ProjectEvent>;

/// Type alias for project event message.
pub type ProjectEventMessage = TypedMessage<ProjectEvent>;

/// Type alias for project event message stream.
pub type ProjectEventStream = TypedMessageStream<ProjectEvent>;
