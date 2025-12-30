//! Workspace event stream subscriber for real-time WebSocket communication.

use async_nats::jetstream::Context;
use derive_more::{Deref, DerefMut};
use uuid::Uuid;

use super::workspace_event::WorkspaceEvent;
use super::subscriber::{StreamSubscriber, TypedBatchStream, TypedMessage, TypedMessageStream};
use crate::Result;

/// Workspace event subscriber for receiving WebSocket messages.
#[derive(Debug, Clone, Deref, DerefMut)]
pub struct WorkspaceEventSubscriber {
    #[deref]
    #[deref_mut]
    subscriber: StreamSubscriber<WorkspaceEvent>,
}

impl WorkspaceEventSubscriber {
    /// Create a new workspace event subscriber.
    ///
    /// # Arguments
    ///
    /// * `jetstream` - JetStream context
    /// * `consumer_name` - Unique name for this consumer (e.g., "server-instance-1")
    pub async fn new(jetstream: &Context, consumer_name: &str) -> Result<Self> {
        let subscriber = StreamSubscriber::new(jetstream, "PROJECT_EVENTS", consumer_name).await?;
        Ok(Self { subscriber })
    }

    /// Create a subscriber filtered to a specific workspace.
    ///
    /// Only receives events for the specified workspace ID.
    pub async fn new_for_workspace(
        jetstream: &Context,
        consumer_name: &str,
        workspace_id: Uuid,
    ) -> Result<Self> {
        let subscriber = StreamSubscriber::new(jetstream, "PROJECT_EVENTS", consumer_name)
            .await?
            .with_filter_subject(format!("PROJECT_EVENTS.{}", workspace_id));
        Ok(Self { subscriber })
    }
}

/// Type alias for workspace event batch stream.
pub type WorkspaceEventBatchStream = TypedBatchStream<WorkspaceEvent>;

/// Type alias for workspace event message.
pub type WorkspaceEventMessage = TypedMessage<WorkspaceEvent>;

/// Type alias for workspace event message stream.
pub type WorkspaceEventStream = TypedMessageStream<WorkspaceEvent>;
