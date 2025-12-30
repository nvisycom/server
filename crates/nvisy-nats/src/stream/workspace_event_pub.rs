//! Workspace event stream publisher for real-time WebSocket communication.

use async_nats::jetstream::Context;
use derive_more::{Deref, DerefMut};
use uuid::Uuid;

use super::workspace_event::{WorkspaceEvent, WorkspaceWsMessage};
use super::publisher::StreamPublisher;
use crate::Result;

/// Workspace event publisher for broadcasting WebSocket messages.
#[derive(Debug, Clone, Deref, DerefMut)]
pub struct WorkspaceEventPublisher {
    #[deref]
    #[deref_mut]
    publisher: StreamPublisher<WorkspaceEvent>,
}

impl WorkspaceEventPublisher {
    /// Create a new workspace event publisher.
    pub async fn new(jetstream: &Context) -> Result<Self> {
        let publisher = StreamPublisher::new(jetstream, "PROJECT_EVENTS").await?;
        Ok(Self { publisher })
    }

    /// Publish a WebSocket message to a specific workspace.
    ///
    /// Messages are published to the subject `PROJECT_EVENTS.{workspace_id}`.
    pub async fn publish_message(&self, workspace_id: Uuid, message: WorkspaceWsMessage) -> Result<()> {
        let event = WorkspaceEvent::new(workspace_id, message);
        let subject = workspace_id.to_string();
        self.publisher.publish(&subject, &event).await
    }

    /// Publish multiple messages to a workspace in batch.
    pub async fn publish_batch(
        &self,
        workspace_id: Uuid,
        messages: Vec<WorkspaceWsMessage>,
    ) -> Result<()> {
        let events: Vec<WorkspaceEvent> = messages
            .into_iter()
            .map(|msg| WorkspaceEvent::new(workspace_id, msg))
            .collect();

        let subject = workspace_id.to_string();
        self.publisher
            .publish_batch_parallel(&subject, &events, 10)
            .await
    }
}
