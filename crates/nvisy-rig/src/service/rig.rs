//! Unified AI service combining chat and RAG capabilities.

use std::sync::Arc;

use nvisy_nats::NatsClient;
use nvisy_postgres::PgClient;

use super::RigConfig;
use crate::Result;
use crate::chat::ChatService;
use crate::rag::{RagConfig, RagService};

/// Inner state for [`RigService`].
struct RigServiceInner {
    chat: ChatService,
    rag: RagService,
}

/// Unified AI service providing chat and RAG capabilities.
#[derive(Clone)]
pub struct RigService {
    inner: Arc<RigServiceInner>,
}

impl RigService {
    /// Creates a new RigService from configuration.
    pub async fn new(config: RigConfig, db: PgClient, nats: NatsClient) -> Result<Self> {
        let embedding_provider = config.embedding_provider()?;

        let rag_config = RagConfig::default();
        let rag = RagService::new(rag_config, embedding_provider.clone(), db, nats.clone()).await?;

        let chat = ChatService::new(embedding_provider, nats).await?;

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
