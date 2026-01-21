//! Streaming chat response.

use std::pin::Pin;
use std::task::{Context, Poll};

use futures::Stream;
use uuid::Uuid;

use super::{ChatEvent, ChatResponse, ChatService, UsageStats};
use crate::Result;
use crate::provider::CompletionModel;
use crate::session::Session;
use crate::tool::edit::ProposedEdit;

/// Streaming chat response.
pub struct ChatStream {
    session: Session,
    message: String,
    model_override: Option<CompletionModel>,
    service: ChatService,

    started: bool,
    finished: bool,
    accumulated_content: String,
    proposed_edits: Vec<ProposedEdit>,
    applied_edits: Vec<Uuid>,
}

impl ChatStream {
    /// Creates a new chat stream.
    pub async fn new(session: Session, message: String, service: ChatService) -> Result<Self> {
        Ok(Self {
            session,
            message,
            model_override: None,
            service,
            started: false,
            finished: false,
            accumulated_content: String::new(),
            proposed_edits: Vec::new(),
            applied_edits: Vec::new(),
        })
    }

    /// Creates a new chat stream with a model override.
    pub async fn with_model(
        session: Session,
        message: String,
        model_override: Option<CompletionModel>,
        service: ChatService,
    ) -> Result<Self> {
        Ok(Self {
            session,
            message,
            model_override,
            service,
            started: false,
            finished: false,
            accumulated_content: String::new(),
            proposed_edits: Vec::new(),
            applied_edits: Vec::new(),
        })
    }

    /// Returns the session ID.
    pub fn session_id(&self) -> Uuid {
        self.session.id()
    }

    /// Returns the document ID being processed.
    pub fn document_id(&self) -> Uuid {
        self.session.document_id()
    }

    fn poll_next_event(&mut self, _cx: &mut Context<'_>) -> Poll<Option<Result<ChatEvent>>> {
        if self.finished {
            return Poll::Ready(None);
        }

        if !self.started {
            self.started = true;

            let _ = (&self.message, &self.service, &self.accumulated_content);

            self.finished = true;

            let model = self
                .model_override
                .as_ref()
                .map(|m| m.as_str().to_string())
                .unwrap_or_else(|| "default".to_string());

            let response = ChatResponse::new(
                "Agent pipeline not yet implemented".to_string(),
                model,
                UsageStats::default(),
            )
            .with_proposed_edits(self.proposed_edits.clone())
            .with_applied_edits(self.applied_edits.clone());

            return Poll::Ready(Some(Ok(ChatEvent::Done { response })));
        }

        Poll::Ready(None)
    }
}

impl Stream for ChatStream {
    type Item = Result<ChatEvent>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.poll_next_event(cx)
    }
}
