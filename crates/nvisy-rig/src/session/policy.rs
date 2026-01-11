//! Auto-apply policies for edit approval.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Context for auto-apply decisions.
#[derive(Debug, Clone)]
pub struct AutoApplyContext {
    /// Number of edits already auto-applied in this session.
    pub auto_applied_count: usize,

    /// Whether the edit is idempotent.
    pub is_idempotent: bool,

    /// The operation type being evaluated (e.g., "replace", "insert", "delete").
    pub operation_type: String,

    /// Number of times user has approved this operation type in this session.
    pub approval_count_for_type: usize,
}

impl AutoApplyContext {
    /// Creates a new context for auto-apply evaluation.
    pub fn new(operation_type: impl Into<String>) -> Self {
        Self {
            auto_applied_count: 0,
            is_idempotent: false,
            operation_type: operation_type.into(),
            approval_count_for_type: 0,
        }
    }

    /// Sets whether the operation is idempotent.
    pub fn with_idempotent(mut self, is_idempotent: bool) -> Self {
        self.is_idempotent = is_idempotent;
        self
    }

    /// Sets the number of auto-applied edits in the session.
    pub fn with_auto_applied_count(mut self, count: usize) -> Self {
        self.auto_applied_count = count;
        self
    }

    /// Sets the approval count for this operation type.
    pub fn with_approval_count(mut self, count: usize) -> Self {
        self.approval_count_for_type = count;
        self
    }
}

/// Tracks approval history per operation type.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ApprovalHistory {
    /// Count of approvals per operation type.
    approvals: HashMap<String, usize>,
}

impl ApprovalHistory {
    /// Creates a new empty approval history.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records an approval for the given operation type.
    pub fn record_approval(&mut self, operation_type: &str) {
        *self
            .approvals
            .entry(operation_type.to_string())
            .or_insert(0) += 1;
    }

    /// Returns the approval count for the given operation type.
    pub fn approval_count(&self, operation_type: &str) -> usize {
        self.approvals.get(operation_type).copied().unwrap_or(0)
    }

    /// Clears all approval history.
    pub fn clear(&mut self) {
        self.approvals.clear();
    }
}

/// Policy for automatically applying edits.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum ApplyPolicy {
    /// Never auto-apply, always require approval.
    #[default]
    RequireApproval,

    /// Auto-apply idempotent operations only.
    IdempotentOnly,

    /// Auto-apply after user approves similar operations.
    LearnFromApproval,

    /// Auto-apply all edits (dangerous).
    AutoApplyAll,

    /// Custom policy with specific rules.
    Custom(CustomPolicy),
}

impl ApplyPolicy {
    /// Creates a policy that requires approval for everything.
    pub fn require_approval() -> Self {
        Self::RequireApproval
    }

    /// Creates a policy that auto-applies idempotent operations.
    pub fn idempotent_only() -> Self {
        Self::IdempotentOnly
    }

    /// Creates a policy that learns from user approvals.
    pub fn learn_from_approval() -> Self {
        Self::LearnFromApproval
    }

    /// Creates a policy that auto-applies everything.
    ///
    /// # Warning
    /// This is dangerous and should only be used for testing
    /// or when the user explicitly opts in.
    pub fn auto_apply_all() -> Self {
        Self::AutoApplyAll
    }

    /// Determines if an edit should be auto-applied.
    pub fn should_auto_apply(&self, context: &AutoApplyContext) -> bool {
        match self {
            Self::RequireApproval => false,
            Self::IdempotentOnly => context.is_idempotent,
            Self::LearnFromApproval => {
                // Auto-apply if idempotent OR if user has approved at least one similar edit
                context.is_idempotent || context.approval_count_for_type > 0
            }
            Self::AutoApplyAll => true,
            Self::Custom(policy) => policy.should_auto_apply(context),
        }
    }
}

/// Custom auto-apply policy with fine-grained rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomPolicy {
    /// Auto-apply idempotent operations.
    pub auto_apply_idempotent: bool,

    /// Auto-apply after N similar approvals for the same operation type.
    pub learn_threshold: Option<usize>,

    /// Maximum edits to auto-apply per session.
    pub max_auto_apply: Option<usize>,

    /// Allowed operation types for auto-apply.
    /// If empty, all operation types are considered.
    pub allowed_operations: Vec<String>,
}

impl CustomPolicy {
    /// Creates a custom policy that auto-applies idempotent operations.
    pub fn idempotent_only() -> Self {
        Self {
            auto_apply_idempotent: true,
            learn_threshold: None,
            max_auto_apply: None,
            allowed_operations: Vec::new(),
        }
    }

    /// Creates a custom policy that learns from approvals.
    pub fn learning(threshold: usize) -> Self {
        Self {
            auto_apply_idempotent: true,
            learn_threshold: Some(threshold),
            max_auto_apply: None,
            allowed_operations: Vec::new(),
        }
    }

    /// Sets the maximum number of auto-applied edits.
    pub fn with_max_auto_apply(mut self, max: usize) -> Self {
        self.max_auto_apply = Some(max);
        self
    }

    /// Sets the allowed operation types.
    pub fn with_allowed_operations(mut self, operations: Vec<String>) -> Self {
        self.allowed_operations = operations;
        self
    }

    /// Determines if an edit should be auto-applied.
    pub fn should_auto_apply(&self, context: &AutoApplyContext) -> bool {
        // Check max auto-apply limit
        if let Some(max) = self.max_auto_apply
            && context.auto_applied_count >= max
        {
            return false;
        }

        // Check if operation type is allowed (empty = all allowed)
        if !self.allowed_operations.is_empty()
            && !self.allowed_operations.contains(&context.operation_type)
        {
            return false;
        }

        // Check idempotent rule
        if self.auto_apply_idempotent && context.is_idempotent {
            return true;
        }

        // Check learn threshold
        if let Some(threshold) = self.learn_threshold
            && context.approval_count_for_type >= threshold
        {
            return true;
        }

        false
    }
}

impl Default for CustomPolicy {
    fn default() -> Self {
        Self {
            auto_apply_idempotent: true,
            learn_threshold: Some(2),
            max_auto_apply: Some(10),
            allowed_operations: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context_for(op_type: &str) -> AutoApplyContext {
        AutoApplyContext::new(op_type)
    }

    #[test]
    fn require_approval_never_auto_applies() {
        let policy = ApplyPolicy::require_approval();
        let context = context_for("replace")
            .with_idempotent(true)
            .with_approval_count(10);

        assert!(!policy.should_auto_apply(&context));
    }

    #[test]
    fn idempotent_only_checks_idempotency() {
        let policy = ApplyPolicy::idempotent_only();

        let idempotent = context_for("replace").with_idempotent(true);
        let non_idempotent = context_for("replace").with_idempotent(false);

        assert!(policy.should_auto_apply(&idempotent));
        assert!(!policy.should_auto_apply(&non_idempotent));
    }

    #[test]
    fn learn_from_approval() {
        let policy = ApplyPolicy::learn_from_approval();

        // Non-idempotent with no approvals - should not auto-apply
        let no_approvals = context_for("delete");
        assert!(!policy.should_auto_apply(&no_approvals));

        // Non-idempotent with approvals - should auto-apply
        let with_approvals = context_for("delete").with_approval_count(1);
        assert!(policy.should_auto_apply(&with_approvals));

        // Idempotent without approvals - should still auto-apply
        let idempotent = context_for("insert").with_idempotent(true);
        assert!(policy.should_auto_apply(&idempotent));
    }

    #[test]
    fn auto_apply_all() {
        let policy = ApplyPolicy::auto_apply_all();
        let context = context_for("delete");

        assert!(policy.should_auto_apply(&context));
    }

    #[test]
    fn custom_policy_max_limit() {
        let policy = CustomPolicy::default().with_max_auto_apply(5);

        // Under limit
        let under_limit = context_for("replace")
            .with_idempotent(true)
            .with_auto_applied_count(4);
        assert!(policy.should_auto_apply(&under_limit));

        // At limit
        let at_limit = context_for("replace")
            .with_idempotent(true)
            .with_auto_applied_count(5);
        assert!(!policy.should_auto_apply(&at_limit));
    }

    #[test]
    fn custom_policy_learn_threshold() {
        let policy = CustomPolicy::learning(3);

        // Below threshold
        let below = context_for("delete").with_approval_count(2);
        assert!(!policy.should_auto_apply(&below));

        // At threshold
        let at_threshold = context_for("delete").with_approval_count(3);
        assert!(policy.should_auto_apply(&at_threshold));
    }

    #[test]
    fn custom_policy_allowed_operations() {
        let policy = CustomPolicy::idempotent_only()
            .with_allowed_operations(vec!["insert".to_string(), "replace".to_string()]);

        // Allowed operation
        let allowed = context_for("insert").with_idempotent(true);
        assert!(policy.should_auto_apply(&allowed));

        // Disallowed operation
        let disallowed = context_for("delete").with_idempotent(true);
        assert!(!policy.should_auto_apply(&disallowed));
    }

    #[test]
    fn approval_history_tracking() {
        let mut history = ApprovalHistory::new();

        assert_eq!(history.approval_count("replace"), 0);

        history.record_approval("replace");
        assert_eq!(history.approval_count("replace"), 1);

        history.record_approval("replace");
        history.record_approval("insert");
        assert_eq!(history.approval_count("replace"), 2);
        assert_eq!(history.approval_count("insert"), 1);
        assert_eq!(history.approval_count("delete"), 0);
    }
}
