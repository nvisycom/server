//! Workspace import job stream subscriber.

use async_nats::jetstream::Context;
use derive_more::{Deref, DerefMut};

use super::subscriber::{StreamSubscriber, TypedBatchStream, TypedMessage, TypedMessageStream};
use super::workspace_import::WorkspaceImportJob;
use crate::Result;

/// Workspace import job subscriber wrapping the base StreamSubscriber.
#[derive(Debug, Clone, Deref, DerefMut)]
pub struct WorkspaceImportSubscriber {
    #[deref]
    #[deref_mut]
    subscriber: StreamSubscriber<WorkspaceImportJob>,
}

impl WorkspaceImportSubscriber {
    /// Create a new workspace import job subscriber.
    pub async fn new(jetstream: &Context, consumer_name: &str) -> Result<Self> {
        let subscriber = StreamSubscriber::new(jetstream, "PROJECT_IMPORTS", consumer_name).await?;
        Ok(Self { subscriber })
    }
}

/// Type alias for workspace import batch stream.
pub type WorkspaceImportBatchStream = TypedBatchStream<WorkspaceImportJob>;

/// Type alias for workspace import message.
pub type WorkspaceImportMessage = TypedMessage<WorkspaceImportJob>;

/// Type alias for workspace import message stream.
pub type WorkspaceImportStream = TypedMessageStream<WorkspaceImportJob>;
