//! Workspace export job stream publisher.

use async_nats::jetstream::Context;
use derive_more::{Deref, DerefMut};

use super::publisher::StreamPublisher;
use super::workspace_export::WorkspaceExportJob;
use crate::Result;

/// Workspace export job publisher wrapping the base StreamPublisher
#[derive(Debug, Clone, Deref, DerefMut)]
pub struct WorkspaceExportPublisher {
    #[deref]
    #[deref_mut]
    publisher: StreamPublisher<WorkspaceExportJob>,
}

impl WorkspaceExportPublisher {
    /// Create a new workspace export job publisher
    pub async fn new(jetstream: &Context) -> Result<Self> {
        let publisher = StreamPublisher::new(jetstream, "PROJECT_EXPORTS").await?;
        Ok(Self { publisher })
    }
}
