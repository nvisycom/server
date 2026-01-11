//! Session management for chat conversations.
//!
//! Sessions are ephemeral and stored in NATS KV with TTL.
//! They track conversation history, pending edits, and auto-apply policies.
//!
//! ## Submodules
//!
//! - [`agent`] - Agent execution within sessions

pub mod agent;
mod message;
mod policy;
mod store;

use chrono::{DateTime, Utc};
pub use message::{Message, MessageRole};
pub use policy::{ApplyPolicy, ApprovalHistory, AutoApplyContext};
use serde::{Deserialize, Serialize};
pub use store::SessionStore;
use uuid::Uuid;

use crate::Result;
use crate::tool::edit::{ApplyResult, ProposedEdit};

/// Request to create a new session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSession {
    /// Document being processed.
    pub document_id: Uuid,

    /// Workspace context.
    pub workspace_id: Uuid,

    /// User initiating the session.
    pub user_id: Uuid,

    /// Auto-apply policy for edits.
    #[serde(default)]
    pub apply_policy: ApplyPolicy,

    /// Initial system prompt override.
    pub system_prompt: Option<String>,

    /// Model preference.
    pub model: Option<String>,
}

impl CreateSession {
    /// Creates a new session request.
    pub fn new(document_id: Uuid, workspace_id: Uuid, user_id: Uuid) -> Self {
        Self {
            document_id,
            workspace_id,
            user_id,
            apply_policy: ApplyPolicy::default(),
            system_prompt: None,
            model: None,
        }
    }

    /// Sets the auto-apply policy.
    pub fn with_policy(mut self, policy: ApplyPolicy) -> Self {
        self.apply_policy = policy;
        self
    }

    /// Sets a custom system prompt.
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    /// Sets a model preference.
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }
}

/// An active chat session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Unique session ID.
    id: Uuid,

    /// Document being processed.
    document_id: Uuid,

    /// Workspace context.
    workspace_id: Uuid,

    /// User who created the session.
    user_id: Uuid,

    /// Auto-apply policy.
    apply_policy: ApplyPolicy,

    /// Custom system prompt.
    system_prompt: Option<String>,

    /// Model preference.
    model: Option<String>,

    /// Conversation history.
    messages: Vec<Message>,

    /// Pending edits awaiting approval.
    pending_edits: Vec<ProposedEdit>,

    /// Applied edit IDs.
    applied_edits: Vec<Uuid>,

    /// Rejected edit IDs.
    rejected_edits: Vec<Uuid>,

    /// Count of auto-applied edits in this session.
    auto_applied_count: usize,

    /// Approval history for learning policies.
    approval_history: ApprovalHistory,

    /// When the session was created.
    created_at: DateTime<Utc>,

    /// Last activity time.
    last_activity_at: DateTime<Utc>,
}

impl Session {
    /// Creates a new session from a request.
    pub fn new(request: CreateSession) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::now_v7(),
            document_id: request.document_id,
            workspace_id: request.workspace_id,
            user_id: request.user_id,
            apply_policy: request.apply_policy,
            system_prompt: request.system_prompt,
            model: request.model,
            messages: Vec::new(),
            pending_edits: Vec::new(),
            applied_edits: Vec::new(),
            rejected_edits: Vec::new(),
            auto_applied_count: 0,
            approval_history: ApprovalHistory::new(),
            created_at: now,
            last_activity_at: now,
        }
    }

    /// Returns the session ID.
    pub fn id(&self) -> Uuid {
        self.id
    }

    /// Returns the document ID.
    pub fn document_id(&self) -> Uuid {
        self.document_id
    }

    /// Returns the workspace ID.
    pub fn workspace_id(&self) -> Uuid {
        self.workspace_id
    }

    /// Returns the user ID.
    pub fn user_id(&self) -> Uuid {
        self.user_id
    }

    /// Returns the auto-apply policy.
    pub fn apply_policy(&self) -> &ApplyPolicy {
        &self.apply_policy
    }

    /// Returns the custom system prompt.
    pub fn system_prompt(&self) -> Option<&str> {
        self.system_prompt.as_deref()
    }

    /// Returns the model preference.
    pub fn model(&self) -> Option<&str> {
        self.model.as_deref()
    }

    /// Returns the conversation messages.
    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    /// Returns pending edits.
    pub fn pending_edits(&self) -> &[ProposedEdit] {
        &self.pending_edits
    }

    /// Returns applied edit IDs.
    pub fn applied_edits(&self) -> &[Uuid] {
        &self.applied_edits
    }

    /// Returns the creation time.
    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    /// Returns the last activity time.
    pub fn last_activity_at(&self) -> DateTime<Utc> {
        self.last_activity_at
    }

    /// Adds a user message.
    pub fn add_user_message(&mut self, content: impl Into<String>) {
        self.messages.push(Message::user(content));
        self.last_activity_at = Utc::now();
    }

    /// Adds an assistant message.
    pub fn add_assistant_message(&mut self, content: impl Into<String>) {
        self.messages.push(Message::assistant(content));
        self.last_activity_at = Utc::now();
    }

    /// Adds a tool result message.
    pub fn add_tool_message(&mut self, tool_call_id: Uuid, content: impl Into<String>) {
        self.messages.push(Message::tool(tool_call_id, content));
        self.last_activity_at = Utc::now();
    }

    /// Adds a proposed edit.
    pub fn add_proposed_edit(&mut self, edit: ProposedEdit) {
        self.pending_edits.push(edit);
        self.last_activity_at = Utc::now();
    }

    /// Checks if an edit should be auto-applied.
    pub fn should_auto_apply(&self, edit: &ProposedEdit) -> bool {
        let op_type = edit.operation_type();
        let context = AutoApplyContext::new(op_type)
            .with_idempotent(edit.is_idempotent())
            .with_auto_applied_count(self.auto_applied_count)
            .with_approval_count(self.approval_history.approval_count(op_type));

        self.apply_policy.should_auto_apply(&context)
    }

    /// Records that an edit was auto-applied.
    pub fn record_auto_apply(&mut self) {
        self.auto_applied_count += 1;
    }

    /// Returns the approval history.
    pub fn approval_history(&self) -> &ApprovalHistory {
        &self.approval_history
    }

    /// Applies pending edits by ID.
    ///
    /// This also records the approval in the history for learning policies.
    pub fn apply_edits(&mut self, edit_ids: &[Uuid]) -> Result<ApplyResult> {
        let mut applied = Vec::new();
        let mut not_found = Vec::new();

        for id in edit_ids {
            if let Some(pos) = self.pending_edits.iter().position(|e| e.id() == *id) {
                let edit = self.pending_edits.remove(pos);
                // Record approval for learning policies
                self.approval_history.record_approval(edit.operation_type());
                applied.push(edit);
                self.applied_edits.push(*id);
            } else {
                not_found.push(*id);
            }
        }

        self.last_activity_at = Utc::now();

        Ok(ApplyResult {
            applied,
            not_found,
            errors: Vec::new(),
        })
    }

    /// Rejects pending edits by ID.
    pub fn reject_edits(&mut self, edit_ids: &[Uuid]) {
        for id in edit_ids {
            if let Some(pos) = self.pending_edits.iter().position(|e| e.id() == *id) {
                self.pending_edits.remove(pos);
                self.rejected_edits.push(*id);
            }
        }
        self.last_activity_at = Utc::now();
    }

    /// Touches the session to update last activity time.
    pub fn touch(&mut self) {
        self.last_activity_at = Utc::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_request() -> CreateSession {
        CreateSession::new(Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7())
    }

    #[test]
    fn session_creation() {
        let session = Session::new(test_request());
        assert!(!session.id().is_nil());
        assert!(session.messages().is_empty());
        assert!(session.pending_edits().is_empty());
    }

    #[test]
    fn session_add_messages() {
        let mut session = Session::new(test_request());

        session.add_user_message("Hello");
        session.add_assistant_message("Hi there!");

        assert_eq!(session.messages().len(), 2);
        assert_eq!(session.messages()[0].role(), MessageRole::User);
        assert_eq!(session.messages()[1].role(), MessageRole::Assistant);
    }
}
