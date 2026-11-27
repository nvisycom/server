//! Detection request types (detect and redact).

use elide_pipeline::entity::{EditSet, LabelCatalog, LabelRef};
use elide_pipeline::provider::DocumentContext;
use garde::Validate;
use nvisy_postgres::types::{DetectionFilter, DetectionStatus, RetentionOverride};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::input::{CreateDetectionInput, DerivedPolicyInput, DetectionSourceInput};

/// Query parameters for listing detections across a workspace.
///
/// Every field is an optional filter; unset fields impose no constraint.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDetectionsQuery {
    /// Case-insensitive substring search of the input display name or original filename.
    pub search: Option<String>,
    /// Filter by detection status.
    pub status: Option<DetectionStatus>,
    /// Filter by the source document the detection analyzes.
    pub document_id: Option<Uuid>,
    /// Filter to detections that ran against a specific policy (any version).
    pub policy_id: Option<Uuid>,
    /// Filter by the account that triggered the detection.
    pub triggered_by: Option<Uuid>,
}

impl From<WorkspaceDetectionsQuery> for DetectionFilter {
    fn from(query: WorkspaceDetectionsQuery) -> Self {
        DetectionFilter {
            search: query.search,
            status: query.status,
            input_document_id: query.document_id,
            policy_id: query.policy_id,
            account_id: query.triggered_by,
        }
    }
}

/// What a detection runs against: exactly one source, enforced by the type.
///
/// Tagged by `source`: `{ "source": "policies", "policyIds": [ ... ] }` or
/// `{ "source": "labels", "labels": [ ... ] }`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "source", rename_all = "camelCase")]
pub enum DetectionSource {
    /// Workspace policies, by id (authored or derived). 1 to 128 of them.
    #[serde(rename_all = "camelCase")]
    Policies {
        /// The policies to run against.
        policy_ids: Vec<Uuid>,
    },
    /// Built-in labels to detect and remove (e.g. `person_name`,
    /// `email_address`), 1 to 128 of them. The server runs them as a derived
    /// policy, reusing an identical one, so detecting by labels needs only the
    /// permission to run detections.
    Labels {
        /// The built-in labels to detect and remove.
        labels: Vec<String>,
    },
}

impl From<DetectionSource> for DetectionSourceInput {
    fn from(source: DetectionSource) -> Self {
        match source {
            DetectionSource::Policies { policy_ids } => {
                DetectionSourceInput::Policies { policy_ids }
            }
            DetectionSource::Labels { labels } => {
                DetectionSourceInput::Labels(DerivedPolicyInput { labels })
            }
        }
    }
}

/// The most policies or labels a detection names.
const MAX_SOURCE_ITEMS: usize = 128;

/// A source names 1 to [`MAX_SOURCE_ITEMS`] policies or labels, and every label is
/// a built-in one.
// reason: signature is fixed by garde's custom validator interface.
#[allow(clippy::trivially_copy_pass_by_ref)]
fn validate_source(source: &DetectionSource, _: &()) -> garde::Result {
    let (what, count) = match source {
        DetectionSource::Policies { policy_ids } => ("policy", policy_ids.len()),
        DetectionSource::Labels { labels } => ("label", labels.len()),
    };
    if count == 0 {
        return Err(garde::Error::new(format!(
            "at least one {what} is required"
        )));
    }
    if count > MAX_SOURCE_ITEMS {
        return Err(garde::Error::new(format!(
            "at most {MAX_SOURCE_ITEMS} {what} entries are allowed"
        )));
    }
    if let DetectionSource::Labels { labels } = source {
        let catalog = LabelCatalog::with_builtins();
        let unknown: Vec<&str> = labels
            .iter()
            .filter(|id| !catalog.contains(&LabelRef::new(id.as_str())))
            .map(String::as_str)
            .collect();
        if !unknown.is_empty() {
            return Err(garde::Error::new(format!(
                "unknown labels: {}",
                unknown.join(", ")
            )));
        }
    }
    Ok(())
}

/// Request payload to start a detection over a document, against named policies
/// or a list of built-in labels.
///
/// Analyzes the document and returns the detection, which holds the findings for
/// review before redaction.
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
#[garde(allow_unvalidated)]
pub struct CreateWorkspaceDetection {
    /// The document to analyze.
    pub document_id: Uuid,
    /// What the detection runs against.
    #[serde(flatten)]
    #[garde(custom(validate_source))]
    pub source: DetectionSource,
    /// Per-document scope (languages, jurisdictions, document labels).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<DocumentContext>,
    /// Retention override for the outputs this detection produces. Absent falls
    /// back to the workspace retention baseline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retention_override: Option<RetentionOverride>,
}

impl From<CreateWorkspaceDetection> for CreateDetectionInput {
    fn from(request: CreateWorkspaceDetection) -> Self {
        CreateDetectionInput {
            document_id: request.document_id,
            source: request.source.into(),
            scope: request.scope,
            retention_override: request.retention_override,
        }
    }
}

/// Request payload to set a detection's accumulated reviewer edits.
///
/// The edits (suppress a false positive, retag a detection, add one the analysis
/// missed) persist on the detection and are layered over its base analysis at redact
/// time, so they survive between redactions. Omit `edits` to clear them.
#[must_use]
#[derive(Debug, Clone, Default, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
#[garde(allow_unvalidated)]
pub struct EditWorkspaceDetection {
    /// The `editSetRev` the client last read for this detection. Required, so the
    /// guard is honest: the write is applied only if it still matches (a concurrent
    /// edit is not silently overwritten, and a caller cannot skip the check by
    /// omitting the field); a stale value is rejected with 409. A never-edited
    /// detection is at rev 0.
    pub edit_set_rev: i32,
    /// The reviewer edits to persist, grouped by modality. Omit to clear the
    /// detection's stored edits.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edits: Option<EditSet>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(ids: &[&str]) -> DetectionSource {
        DetectionSource::Labels {
            labels: ids.iter().map(|id| (*id).to_owned()).collect(),
        }
    }

    #[test]
    fn a_source_accepts_known_labels_and_rejects_unknown_ones() {
        assert!(validate_source(&labels(&["person_name", "email_address"]), &()).is_ok());

        let err = validate_source(&labels(&["person_name", "not_a_real_label"]), &())
            .expect_err("unknown label is rejected");
        assert!(err.to_string().contains("not_a_real_label"));
    }

    #[test]
    fn a_source_must_name_something() {
        assert!(validate_source(&labels(&[]), &()).is_err());
        let none = DetectionSource::Policies {
            policy_ids: Vec::new(),
        };
        assert!(validate_source(&none, &()).is_err());
    }
}
