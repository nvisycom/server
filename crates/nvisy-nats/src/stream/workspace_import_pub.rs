//! Workspace import job stream publisher.

use async_nats::jetstream::Context;
use derive_more::{Deref, DerefMut};

use super::workspace_import::WorkspaceImportJob;
use super::publisher::StreamPublisher;
use crate::Result;

/// Workspace import job publisher wrapping the base StreamPublisher
#[derive(Debug, Clone, Deref, DerefMut)]
pub struct WorkspaceImportPublisher {
    #[deref]
    #[deref_mut]
    publisher: StreamPublisher<WorkspaceImportJob>,
}

impl WorkspaceImportPublisher {
    /// Create a new workspace import job publisher
    pub async fn new(jetstream: &Context) -> Result<Self> {
        let publisher = StreamPublisher::new(jetstream, "PROJECT_IMPORTS").await?;
        Ok(Self { publisher })
    }
}
