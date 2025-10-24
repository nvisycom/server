//! Project import job stream publisher.

use derive_more::{Deref, DerefMut};

use super::project_import::ProjectImportJob;
use super::publisher::StreamPublisher;
use crate::Result;

/// Project import job publisher wrapping the base StreamPublisher
#[derive(Debug, Clone, Deref, DerefMut)]
pub struct ProjectImportPublisher {
    #[deref]
    #[deref_mut]
    publisher: StreamPublisher<ProjectImportJob>,
}

impl ProjectImportPublisher {
    /// Create a new project import job publisher
    pub async fn new(jetstream: &async_nats::jetstream::Context) -> Result<Self> {
        let publisher = StreamPublisher::new(jetstream, "PROJECT_IMPORTS").await?;
        Ok(Self { publisher })
    }
}
