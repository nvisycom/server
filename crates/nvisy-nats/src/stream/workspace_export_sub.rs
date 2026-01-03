//! Workspace export job stream subscriber.

use async_nats::jetstream::Context;
use derive_more::{Deref, DerefMut};

use super::subscriber::{StreamSubscriber, TypedBatchStream, TypedMessage, TypedMessageStream};
use super::workspace_export::WorkspaceExportJob;
use crate::Result;

/// Workspace export job subscriber wrapping the base StreamSubscriber.
#[derive(Debug, Clone, Deref, DerefMut)]
pub struct WorkspaceExportSubscriber {
    #[deref]
    #[deref_mut]
    subscriber: StreamSubscriber<WorkspaceExportJob>,
}

impl WorkspaceExportSubscriber {
    /// Create a new workspace export job subscriber.
    pub async fn new(jetstream: &Context, consumer_name: &str) -> Result<Self> {
        let subscriber = StreamSubscriber::new(jetstream, "PROJECT_EXPORTS", consumer_name).await?;
        Ok(Self { subscriber })
    }
}

/// Type alias for workspace export batch stream.
pub type WorkspaceExportBatchStream = TypedBatchStream<WorkspaceExportJob>;

/// Type alias for workspace export message.
pub type WorkspaceExportMessage = TypedMessage<WorkspaceExportJob>;

/// Type alias for workspace export message stream.
pub type WorkspaceExportStream = TypedMessageStream<WorkspaceExportJob>;
