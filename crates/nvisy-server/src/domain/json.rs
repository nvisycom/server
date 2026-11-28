//! JSON encoding shared by the domain services: values stored as plaintext
//! `JSONB` (policy definitions, edit sets, outbox jobs).

use elide_pipeline::governance::policy::Policy;
use serde::Serialize;
use uuid::Uuid;

use crate::response::{ErrorKind, Result};

/// Encodes `value` for storage; `what` names it in the error.
pub(super) fn encode(value: &impl Serialize, what: &str) -> Result<serde_json::Value> {
    serde_json::to_value(value).map_err(|err| {
        ErrorKind::InternalServerError
            .with_message(format!("Failed to encode the {what}"))
            .with_context(err.to_string())
    })
}

/// Decodes a stored policy version's definition into an engine [`Policy`]. The
/// error names the version, so a malformed row can be found and repaired.
pub(super) fn parse_policy_definition(
    version_id: Uuid,
    definition: serde_json::Value,
) -> Result<Policy> {
    serde_json::from_value(definition).map_err(|err| {
        ErrorKind::InternalServerError
            .with_message("Stored policy definition is malformed")
            .with_context(format!("policy_version_id: {version_id}: {err}"))
    })
}
