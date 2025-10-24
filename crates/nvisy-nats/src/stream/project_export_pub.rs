//! Project export job stream publisher.

use derive_more::{Deref, DerefMut};

use super::project_export::ProjectExportJob;
use super::publisher::StreamPublisher;
use crate::Result;

/// Project export job publisher wrapping the base StreamPublisher
#[derive(Debug, Clone, Deref, DerefMut)]
pub struct ProjectExportPublisher {
    #[deref]
    #[deref_mut]
    publisher: StreamPublisher<ProjectExportJob>,
}

impl ProjectExportPublisher {
    /// Create a new project export job publisher
    pub async fn new(jetstream: &async_nats::jetstream::Context) -> Result<Self> {
        let publisher = StreamPublisher::new(jetstream, "PROJECT_EXPORTS").await?;
        Ok(Self { publisher })
    }
}
