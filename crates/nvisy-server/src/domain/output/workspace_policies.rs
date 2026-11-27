//! Policy service outputs.

use nvisy_postgres::model::{WorkspacePolicy, WorkspacePolicyVersion};

/// A derived policy paired with its current version, and whether this call created
/// it (rather than reusing an identical one).
pub struct ResolvedPolicy {
    /// The policy row.
    pub policy: WorkspacePolicy,
    /// The current version whose definition the engine consumes.
    pub version: WorkspacePolicyVersion,
    /// Whether this call created the policy (vs. reusing an existing derived policy).
    pub created: bool,
}
