//! Project event stream publisher for real-time WebSocket communication.

use async_nats::jetstream::Context;
use derive_more::{Deref, DerefMut};
use uuid::Uuid;

use super::project_event::{ProjectEvent, ProjectWsMessage};
use super::publisher::StreamPublisher;
use crate::Result;

/// Project event publisher for broadcasting WebSocket messages.
#[derive(Debug, Clone, Deref, DerefMut)]
pub struct ProjectEventPublisher {
    #[deref]
    #[deref_mut]
    publisher: StreamPublisher<ProjectEvent>,
}

impl ProjectEventPublisher {
    /// Create a new project event publisher.
    pub async fn new(jetstream: &Context) -> Result<Self> {
        let publisher = StreamPublisher::new(jetstream, "PROJECT_EVENTS").await?;
        Ok(Self { publisher })
    }

    /// Publish a WebSocket message to a specific project.
    ///
    /// Messages are published to the subject `PROJECT_EVENTS.{project_id}`.
    pub async fn publish_message(&self, project_id: Uuid, message: ProjectWsMessage) -> Result<()> {
        let event = ProjectEvent::new(project_id, message);
        let subject = project_id.to_string();
        self.publisher.publish(&subject, &event).await
    }

    /// Publish multiple messages to a project in batch.
    pub async fn publish_batch(
        &self,
        project_id: Uuid,
        messages: Vec<ProjectWsMessage>,
    ) -> Result<()> {
        let events: Vec<ProjectEvent> = messages
            .into_iter()
            .map(|msg| ProjectEvent::new(project_id, msg))
            .collect();

        let subject = project_id.to_string();
        self.publisher
            .publish_batch_parallel(&subject, &events, 10)
            .await
    }
}
