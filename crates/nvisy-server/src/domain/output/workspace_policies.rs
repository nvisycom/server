//! Policy service outputs.

use nvisy_postgres::model::{WorkspacePolicy, WorkspacePolicyVersion};

/// A policy paired with its current version, and whether this call created it.
///
/// `created` distinguishes a freshly minted policy from a reused one-shot, so the
/// handler can answer `201` on creation and `200` on reuse.
pub struct ResolvedPolicy {
    /// The policy row.
    pub policy: WorkspacePolicy,
    /// The current version whose definition the engine consumes.
    pub version: WorkspacePolicyVersion,
    /// Whether this call created the policy (vs. reusing an existing one-shot).
    pub created: bool,
}
