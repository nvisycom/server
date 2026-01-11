//! Unified AI service combining chat and RAG capabilities.

use std::sync::Arc;

use nvisy_nats::NatsClient;
use nvisy_postgres::PgClient;

use super::RigConfig;
use crate::Result;
use crate::chat::ChatService;
use crate::provider::ProviderRegistry;
use crate::rag::{RagConfig, RagService};

/// Inner state for [`RigService`].
struct RigServiceInner {
    chat: ChatService,
    rag: RagService,
}

/// Unified AI service providing chat and RAG capabilities.
///
/// This type is cheap to clone and can be shared across threads.
#[derive(Clone)]
pub struct RigService {
    inner: Arc<RigServiceInner>,
}

impl RigService {
    /// Creates a new RigService from configuration.
    pub async fn new(config: RigConfig, db: PgClient, nats: NatsClient) -> Result<Self> {
        // Initialize RAG service
        let embedding_provider = config.embedding_provider();
        let rag_config = RagConfig::default();
        let rag = RagService::new(rag_config, embedding_provider, db, nats.clone()).await?;

        // Initialize Chat service
        let providers = ProviderRegistry::empty();
        let chat = ChatService::new(providers, nats).await?;

        Ok(Self {
            inner: Arc::new(RigServiceInner { chat, rag }),
        })
    }

    /// Returns a reference to the chat service.
    pub fn chat(&self) -> &ChatService {
        &self.inner.chat
    }

    /// Returns a reference to the RAG service.
    pub fn rag(&self) -> &RagService {
        &self.inner.rag
    }
}
