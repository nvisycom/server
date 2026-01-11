//! Chat response types.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::UsageStats;
use crate::tool::edit::ProposedEdit;

/// Complete chat response after stream ends.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    /// Unique message ID.
    pub id: Uuid,

    /// Complete response text.
    pub content: String,

    /// Model used for completion.
    pub model: String,

    /// Token usage statistics.
    pub usage: UsageStats,

    /// Proposed edits from this response.
    pub proposed_edits: Vec<ProposedEdit>,

    /// Edits that were auto-applied.
    pub applied_edits: Vec<Uuid>,
}

impl ChatResponse {
    /// Creates a new chat response.
    pub fn new(content: String, model: String, usage: UsageStats) -> Self {
        Self {
            id: Uuid::now_v7(),
            content,
            model,
            usage,
            proposed_edits: Vec::new(),
            applied_edits: Vec::new(),
        }
    }

    /// Adds proposed edits to the response.
    pub fn with_proposed_edits(mut self, edits: Vec<ProposedEdit>) -> Self {
        self.proposed_edits = edits;
        self
    }

    /// Adds applied edits to the response.
    pub fn with_applied_edits(mut self, edit_ids: Vec<Uuid>) -> Self {
        self.applied_edits = edit_ids;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_response_builder() {
        let response = ChatResponse::new(
            "Test content".to_string(),
            "gpt-4".to_string(),
            UsageStats::default(),
        );

        assert!(!response.id.is_nil());
        assert_eq!(response.content, "Test content");
        assert_eq!(response.model, "gpt-4");
    }
}
