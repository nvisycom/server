//! The assistant-reply job payload and its work-queue alias.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A queued assistant reply: enough to re-load the conversation and post the
/// reply. The worker re-reads the thread's comments fresh from these ids (rather
/// than carrying the conversation on the wire), so the reply reflects the thread
/// as it stands when the worker runs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantJob {
    /// Workspace the thread belongs to.
    pub workspace_id: Uuid,
    /// Thread the assistant was addressed in.
    pub thread_id: Uuid,
    /// The comment that addressed the assistant (the triggering message).
    pub comment_id: Uuid,
}

/// The assistant work-queue, pinned to the [`AssistantJob`] payload.
pub type AssistantStream = nvisy_nats::stream::AssistantStream<AssistantJob>;
