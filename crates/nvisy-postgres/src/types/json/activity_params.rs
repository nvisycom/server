//! Activity payloads stored in `workspace_activities.params`.
//!
//! An activity stores its `activity_type` (indexed column) plus a self-describing
//! [`Json`](super::Json) body. [`ActivityPayload`] is that body — a
//! `{type, data}`-tagged enum, one variant per event, each carrying its own
//! params. No rendered text is stored; the client localizes copy from `type` and
//! the params.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::types::{
    ActivityType, AssignmentStatus, ConnectionId, DetectionId, Handle, ProviderId, RedactionId,
    WebhookEvent, WebhookId,
};

/// Params of a workspace-scoped activity (`workspace.*`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceActivityParams {
    /// Slug of the workspace acted on.
    pub workspace_slug: Handle,
}

/// Params of a member activity (`member.*`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct MemberActivityParams {
    /// Username of the member acted on.
    pub member_username: Handle,
}

/// Params of an invite activity (`invite.*`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct InviteActivityParams {
    /// Id of the invite.
    pub invite_id: Uuid,
    /// Email the invite was addressed to, when it recorded one. `None` keeps an
    /// absent address distinct from a blank one.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub email: Option<String>,
}

/// Params of a connection activity (`connection.*`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ConnectionActivityParams {
    /// Id of the connection.
    pub connection_id: ConnectionId,
    /// Display name of the connection.
    pub connection_name: String,
}

/// Params of a provider activity (`provider.*`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ProviderActivityParams {
    /// Id of the provider.
    pub provider_id: ProviderId,
    /// Display name of the provider.
    pub provider_name: String,
}

/// Params of a webhook activity (`webhook.*`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct WebhookActivityParams {
    /// Id of the webhook.
    pub webhook_id: WebhookId,
    /// Display name of the webhook.
    pub webhook_name: String,
}

/// Params of a file activity (`file.*`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct FileActivityParams {
    /// Id of the file.
    pub file_id: Uuid,
    /// Display name of the file.
    pub file_name: String,
}

/// Params of a file-assignment activity (`file.assigned`, `file.unassigned`,
/// `file.assignment.updated`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct AssignmentActivityParams {
    /// Id of the assignment.
    pub assignment_id: Uuid,
    /// Display name of the file under review.
    pub file_name: String,
    /// Username of the reviewer the file is assigned to.
    pub assignee_username: Handle,
    /// The reviewer's review status at the time of the activity.
    pub status: AssignmentStatus,
}

/// Params of a pipeline activity (`pipeline.*`, non-run).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct PipelineActivityParams {
    /// Slug of the pipeline.
    pub pipeline_slug: Handle,
}

/// Params of a detection activity (`pipeline.detection.*`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct DetectionActivityParams {
    /// Slug of the owning pipeline.
    pub pipeline_slug: Handle,
    /// Id of the detection.
    pub detection_id: DetectionId,
}

/// Params of a redaction activity (`pipeline.redaction.*`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct RedactionActivityParams {
    /// Slug of the owning pipeline.
    pub pipeline_slug: Handle,
    /// Id of the redaction.
    pub redaction_id: RedactionId,
}

/// Params of a policy activity (`policy.*`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct PolicyActivityParams {
    /// Id of the policy.
    pub policy_id: Uuid,
    /// Slug of the policy.
    pub policy_slug: Handle,
}

/// The typed payload of an audit-log activity, tagged by `type` with its params
/// under `data` (the same `{type, data}` envelope the notification payload and
/// outbox event use).
///
/// Each variant is one activity type carrying its own params. No rendered text is
/// included — the client localizes the copy from `type` and the params. The `type`
/// values match the `ACTIVITY_TYPE` enum.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "type", content = "data")]
pub enum ActivityPayload {
    /// A workspace was created.
    #[serde(rename = "workspace.created")]
    WorkspaceCreated(WorkspaceActivityParams),
    /// A workspace was updated.
    #[serde(rename = "workspace.updated")]
    WorkspaceUpdated(WorkspaceActivityParams),
    /// A workspace was deleted.
    #[serde(rename = "workspace.deleted")]
    WorkspaceDeleted(WorkspaceActivityParams),

    /// A member joined the workspace.
    #[serde(rename = "member.added")]
    MemberAdded(MemberActivityParams),
    /// A member was updated.
    #[serde(rename = "member.updated")]
    MemberUpdated(MemberActivityParams),
    /// A member was removed.
    #[serde(rename = "member.deleted")]
    MemberDeleted(MemberActivityParams),

    /// An invite was created.
    #[serde(rename = "invite.created")]
    InviteCreated(InviteActivityParams),
    /// An invite was accepted.
    #[serde(rename = "invite.accepted")]
    InviteAccepted(InviteActivityParams),
    /// An invite was declined.
    #[serde(rename = "invite.declined")]
    InviteDeclined(InviteActivityParams),
    /// An invite was canceled.
    #[serde(rename = "invite.canceled")]
    InviteCanceled(InviteActivityParams),

    /// A connection was created.
    #[serde(rename = "connection.created")]
    ConnectionCreated(ConnectionActivityParams),
    /// A connection was updated.
    #[serde(rename = "connection.updated")]
    ConnectionUpdated(ConnectionActivityParams),
    /// A connection was deleted.
    #[serde(rename = "connection.deleted")]
    ConnectionDeleted(ConnectionActivityParams),
    /// A connection sync started.
    #[serde(rename = "connection.sync.started")]
    ConnectionSyncStarted(ConnectionActivityParams),
    /// A connection sync completed.
    #[serde(rename = "connection.sync.completed")]
    ConnectionSyncCompleted(ConnectionActivityParams),
    /// A connection sync failed.
    #[serde(rename = "connection.sync.failed")]
    ConnectionSyncFailed(ConnectionActivityParams),

    /// A provider was created.
    #[serde(rename = "provider.created")]
    ProviderCreated(ProviderActivityParams),
    /// A provider was updated.
    #[serde(rename = "provider.updated")]
    ProviderUpdated(ProviderActivityParams),
    /// A provider was deleted.
    #[serde(rename = "provider.deleted")]
    ProviderDeleted(ProviderActivityParams),

    /// A webhook was created.
    #[serde(rename = "webhook.created")]
    WebhookCreated(WebhookActivityParams),
    /// A webhook was updated.
    #[serde(rename = "webhook.updated")]
    WebhookUpdated(WebhookActivityParams),
    /// A webhook was deleted.
    #[serde(rename = "webhook.deleted")]
    WebhookDeleted(WebhookActivityParams),

    /// A file was created.
    #[serde(rename = "file.created")]
    FileCreated(FileActivityParams),
    /// A file was updated.
    #[serde(rename = "file.updated")]
    FileUpdated(FileActivityParams),
    /// A file was deleted.
    #[serde(rename = "file.deleted")]
    FileDeleted(FileActivityParams),
    /// A file was assigned to a reviewer.
    #[serde(rename = "file.assigned")]
    FileAssigned(AssignmentActivityParams),
    /// A reviewer was unassigned from a file.
    #[serde(rename = "file.unassigned")]
    FileUnassigned(AssignmentActivityParams),
    /// A file assignment's review status changed.
    #[serde(rename = "file.assignment.updated")]
    AssignmentStatusChanged(AssignmentActivityParams),

    /// A pipeline was created.
    #[serde(rename = "pipeline.created")]
    PipelineCreated(PipelineActivityParams),
    /// A pipeline was updated.
    #[serde(rename = "pipeline.updated")]
    PipelineUpdated(PipelineActivityParams),
    /// A pipeline was deleted.
    #[serde(rename = "pipeline.deleted")]
    PipelineDeleted(PipelineActivityParams),
    /// A detection was started.
    #[serde(rename = "pipeline.detection.started")]
    DetectionStarted(DetectionActivityParams),
    /// A detection finished analysis.
    #[serde(rename = "pipeline.detection.completed")]
    DetectionCompleted(DetectionActivityParams),
    /// A detection failed.
    #[serde(rename = "pipeline.detection.failed")]
    DetectionFailed(DetectionActivityParams),
    /// A redaction was created.
    #[serde(rename = "pipeline.redaction.created")]
    RedactionCreated(RedactionActivityParams),

    /// A policy was created.
    #[serde(rename = "policy.created")]
    PolicyCreated(PolicyActivityParams),
    /// A policy was updated.
    #[serde(rename = "policy.updated")]
    PolicyUpdated(PolicyActivityParams),
    /// A policy was deleted.
    #[serde(rename = "policy.deleted")]
    PolicyDeleted(PolicyActivityParams),
}

impl ActivityPayload {
    /// The [`ActivityType`] this payload records, so a caller logs an activity
    /// from the payload alone and the two can never disagree.
    pub fn activity_type(&self) -> ActivityType {
        match self {
            ActivityPayload::WorkspaceCreated(_) => ActivityType::WorkspaceCreated,
            ActivityPayload::WorkspaceUpdated(_) => ActivityType::WorkspaceUpdated,
            ActivityPayload::WorkspaceDeleted(_) => ActivityType::WorkspaceDeleted,
            ActivityPayload::MemberAdded(_) => ActivityType::MemberAdded,
            ActivityPayload::MemberUpdated(_) => ActivityType::MemberUpdated,
            ActivityPayload::MemberDeleted(_) => ActivityType::MemberDeleted,
            ActivityPayload::InviteCreated(_) => ActivityType::InviteCreated,
            ActivityPayload::InviteAccepted(_) => ActivityType::InviteAccepted,
            ActivityPayload::InviteDeclined(_) => ActivityType::InviteDeclined,
            ActivityPayload::InviteCanceled(_) => ActivityType::InviteCanceled,
            ActivityPayload::ConnectionCreated(_) => ActivityType::ConnectionCreated,
            ActivityPayload::ConnectionUpdated(_) => ActivityType::ConnectionUpdated,
            ActivityPayload::ConnectionDeleted(_) => ActivityType::ConnectionDeleted,
            ActivityPayload::ConnectionSyncStarted(_) => ActivityType::ConnectionSyncStarted,
            ActivityPayload::ConnectionSyncCompleted(_) => ActivityType::ConnectionSyncCompleted,
            ActivityPayload::ConnectionSyncFailed(_) => ActivityType::ConnectionSyncFailed,
            ActivityPayload::ProviderCreated(_) => ActivityType::ProviderCreated,
            ActivityPayload::ProviderUpdated(_) => ActivityType::ProviderUpdated,
            ActivityPayload::ProviderDeleted(_) => ActivityType::ProviderDeleted,
            ActivityPayload::WebhookCreated(_) => ActivityType::WebhookCreated,
            ActivityPayload::WebhookUpdated(_) => ActivityType::WebhookUpdated,
            ActivityPayload::WebhookDeleted(_) => ActivityType::WebhookDeleted,
            ActivityPayload::FileCreated(_) => ActivityType::FileCreated,
            ActivityPayload::FileUpdated(_) => ActivityType::FileUpdated,
            ActivityPayload::FileDeleted(_) => ActivityType::FileDeleted,
            ActivityPayload::FileAssigned(_) => ActivityType::FileAssigned,
            ActivityPayload::FileUnassigned(_) => ActivityType::FileUnassigned,
            ActivityPayload::AssignmentStatusChanged(_) => ActivityType::AssignmentStatusChanged,
            ActivityPayload::PipelineCreated(_) => ActivityType::PipelineCreated,
            ActivityPayload::PipelineUpdated(_) => ActivityType::PipelineUpdated,
            ActivityPayload::PipelineDeleted(_) => ActivityType::PipelineDeleted,
            ActivityPayload::DetectionStarted(_) => ActivityType::DetectionStarted,
            ActivityPayload::DetectionCompleted(_) => ActivityType::DetectionCompleted,
            ActivityPayload::DetectionFailed(_) => ActivityType::DetectionFailed,
            ActivityPayload::RedactionCreated(_) => ActivityType::RedactionCreated,
            ActivityPayload::PolicyCreated(_) => ActivityType::PolicyCreated,
            ActivityPayload::PolicyUpdated(_) => ActivityType::PolicyUpdated,
            ActivityPayload::PolicyDeleted(_) => ActivityType::PolicyDeleted,
        }
    }

    /// The webhook event this activity also raises, when the webhook vocabulary
    /// carries it. `None` for activities with no webhook counterpart — webhook
    /// CRUD (a webhook does not fire on its own management) and invite lifecycle.
    pub fn webhook_event(&self) -> Option<WebhookEvent> {
        use WebhookEvent as W;
        Some(match self {
            ActivityPayload::WorkspaceCreated(_)
            | ActivityPayload::WorkspaceUpdated(_)
            | ActivityPayload::WorkspaceDeleted(_)
            | ActivityPayload::InviteCreated(_)
            | ActivityPayload::InviteAccepted(_)
            | ActivityPayload::InviteDeclined(_)
            | ActivityPayload::InviteCanceled(_)
            | ActivityPayload::WebhookCreated(_)
            | ActivityPayload::WebhookUpdated(_)
            | ActivityPayload::WebhookDeleted(_) => return None,

            ActivityPayload::MemberAdded(_) => W::MemberAdded,
            ActivityPayload::MemberUpdated(_) => W::MemberUpdated,
            ActivityPayload::MemberDeleted(_) => W::MemberDeleted,
            ActivityPayload::ConnectionCreated(_) => W::ConnectionCreated,
            ActivityPayload::ConnectionUpdated(_) => W::ConnectionUpdated,
            ActivityPayload::ConnectionDeleted(_) => W::ConnectionDeleted,
            ActivityPayload::ConnectionSyncStarted(_) => W::ConnectionSyncStarted,
            ActivityPayload::ConnectionSyncCompleted(_) => W::ConnectionSyncCompleted,
            ActivityPayload::ConnectionSyncFailed(_) => W::ConnectionSyncFailed,
            ActivityPayload::ProviderCreated(_) => W::ProviderCreated,
            ActivityPayload::ProviderUpdated(_) => W::ProviderUpdated,
            ActivityPayload::ProviderDeleted(_) => W::ProviderDeleted,
            ActivityPayload::FileCreated(_) => W::FileCreated,
            ActivityPayload::FileUpdated(_) => W::FileUpdated,
            ActivityPayload::FileDeleted(_) => W::FileDeleted,
            ActivityPayload::FileAssigned(_) => W::FileAssigned,
            ActivityPayload::FileUnassigned(_) => W::FileUnassigned,
            ActivityPayload::AssignmentStatusChanged(_) => W::AssignmentStatusChanged,
            ActivityPayload::PipelineCreated(_) => W::PipelineCreated,
            ActivityPayload::PipelineUpdated(_) => W::PipelineUpdated,
            ActivityPayload::PipelineDeleted(_) => W::PipelineDeleted,
            ActivityPayload::DetectionStarted(_) => W::DetectionStarted,
            ActivityPayload::DetectionCompleted(_) => W::DetectionCompleted,
            ActivityPayload::DetectionFailed(_) => W::DetectionFailed,
            ActivityPayload::RedactionCreated(_) => W::RedactionCreated,
            ActivityPayload::PolicyCreated(_) => W::PolicyCreated,
            ActivityPayload::PolicyUpdated(_) => W::PolicyUpdated,
            ActivityPayload::PolicyDeleted(_) => W::PolicyDeleted,
        })
    }

    /// The stable identifier of the object this activity acted on, when it has
    /// one. `None` for objects addressed only by a human-readable handle (a
    /// workspace, member, or pipeline, whose slug/username is the
    /// [`object_label`](Self::object_label)).
    ///
    /// Paired with `object_label`, this flattens the per-variant params into two
    /// export columns without a per-object-type column explosion.
    pub fn object_id(&self) -> Option<String> {
        match self {
            ActivityPayload::InviteCreated(p)
            | ActivityPayload::InviteAccepted(p)
            | ActivityPayload::InviteDeclined(p)
            | ActivityPayload::InviteCanceled(p) => Some(p.invite_id.to_string()),

            ActivityPayload::ConnectionCreated(p)
            | ActivityPayload::ConnectionUpdated(p)
            | ActivityPayload::ConnectionDeleted(p)
            | ActivityPayload::ConnectionSyncStarted(p)
            | ActivityPayload::ConnectionSyncCompleted(p)
            | ActivityPayload::ConnectionSyncFailed(p) => Some(p.connection_id.to_string()),

            ActivityPayload::ProviderCreated(p)
            | ActivityPayload::ProviderUpdated(p)
            | ActivityPayload::ProviderDeleted(p) => Some(p.provider_id.to_string()),

            ActivityPayload::WebhookCreated(p)
            | ActivityPayload::WebhookUpdated(p)
            | ActivityPayload::WebhookDeleted(p) => Some(p.webhook_id.to_string()),

            ActivityPayload::FileCreated(p)
            | ActivityPayload::FileUpdated(p)
            | ActivityPayload::FileDeleted(p) => Some(p.file_id.to_string()),

            ActivityPayload::FileAssigned(p)
            | ActivityPayload::FileUnassigned(p)
            | ActivityPayload::AssignmentStatusChanged(p) => Some(p.assignment_id.to_string()),

            ActivityPayload::DetectionStarted(p)
            | ActivityPayload::DetectionCompleted(p)
            | ActivityPayload::DetectionFailed(p) => Some(p.detection_id.to_string()),

            ActivityPayload::RedactionCreated(p) => Some(p.redaction_id.to_string()),

            ActivityPayload::PolicyCreated(p)
            | ActivityPayload::PolicyUpdated(p)
            | ActivityPayload::PolicyDeleted(p) => Some(p.policy_id.to_string()),

            ActivityPayload::WorkspaceCreated(_)
            | ActivityPayload::WorkspaceUpdated(_)
            | ActivityPayload::WorkspaceDeleted(_)
            | ActivityPayload::MemberAdded(_)
            | ActivityPayload::MemberUpdated(_)
            | ActivityPayload::MemberDeleted(_)
            | ActivityPayload::PipelineCreated(_)
            | ActivityPayload::PipelineUpdated(_)
            | ActivityPayload::PipelineDeleted(_) => None,
        }
    }

    /// The human-readable name of the object this activity acted on, when it has
    /// one: a slug, username, filename, or email. `None` for objects identified
    /// only by an [`object_id`](Self::object_id).
    pub fn object_label(&self) -> Option<String> {
        match self {
            ActivityPayload::WorkspaceCreated(p)
            | ActivityPayload::WorkspaceUpdated(p)
            | ActivityPayload::WorkspaceDeleted(p) => Some(p.workspace_slug.to_string()),

            ActivityPayload::MemberAdded(p)
            | ActivityPayload::MemberUpdated(p)
            | ActivityPayload::MemberDeleted(p) => Some(p.member_username.to_string()),

            ActivityPayload::InviteCreated(p)
            | ActivityPayload::InviteAccepted(p)
            | ActivityPayload::InviteDeclined(p)
            | ActivityPayload::InviteCanceled(p) => p.email.clone(),

            ActivityPayload::FileCreated(p)
            | ActivityPayload::FileUpdated(p)
            | ActivityPayload::FileDeleted(p) => Some(p.file_name.clone()),

            ActivityPayload::FileAssigned(p)
            | ActivityPayload::FileUnassigned(p)
            | ActivityPayload::AssignmentStatusChanged(p) => Some(p.file_name.clone()),

            ActivityPayload::PipelineCreated(p)
            | ActivityPayload::PipelineUpdated(p)
            | ActivityPayload::PipelineDeleted(p) => Some(p.pipeline_slug.to_string()),

            ActivityPayload::DetectionStarted(p)
            | ActivityPayload::DetectionCompleted(p)
            | ActivityPayload::DetectionFailed(p) => Some(p.pipeline_slug.to_string()),

            ActivityPayload::RedactionCreated(p) => Some(p.pipeline_slug.to_string()),

            ActivityPayload::ConnectionCreated(p)
            | ActivityPayload::ConnectionUpdated(p)
            | ActivityPayload::ConnectionDeleted(p)
            | ActivityPayload::ConnectionSyncStarted(p)
            | ActivityPayload::ConnectionSyncCompleted(p)
            | ActivityPayload::ConnectionSyncFailed(p) => Some(p.connection_name.clone()),

            ActivityPayload::ProviderCreated(p)
            | ActivityPayload::ProviderUpdated(p)
            | ActivityPayload::ProviderDeleted(p) => Some(p.provider_name.clone()),

            ActivityPayload::WebhookCreated(p)
            | ActivityPayload::WebhookUpdated(p)
            | ActivityPayload::WebhookDeleted(p) => Some(p.webhook_name.clone()),

            ActivityPayload::PolicyCreated(p)
            | ActivityPayload::PolicyUpdated(p)
            | ActivityPayload::PolicyDeleted(p) => Some(p.policy_slug.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    #[test]
    fn connection_object_has_prefixed_id_and_display_name_label() {
        let id = Uuid::now_v7();
        let connection_id = ConnectionId::from_uuid(id);
        let p = ActivityPayload::ConnectionCreated(ConnectionActivityParams {
            connection_id,
            connection_name: "Prod S3".to_owned(),
        });
        // `object_id` carries the client-facing prefixed id (`conn_…`), matching
        // how the REST API exposes the same connection.
        assert_eq!(p.object_id(), Some(connection_id.to_string()));
        assert!(p.object_id().unwrap().starts_with("conn_"));
        assert_eq!(p.object_label(), Some("Prod S3".to_owned()));
    }

    #[test]
    fn label_only_object_has_label_no_id() {
        let p = ActivityPayload::MemberAdded(MemberActivityParams {
            member_username: Handle::from_str("alice").unwrap(),
        });
        assert_eq!(p.object_id(), None);
        assert_eq!(p.object_label(), Some("alice".to_owned()));
    }

    /// A small sample of payloads spanning every mapping shape (id-only,
    /// label-only, both) and the provider variants added with the split.
    fn sample_payloads() -> Vec<ActivityPayload> {
        let provider = || ProviderActivityParams {
            provider_id: ProviderId::from_uuid(Uuid::now_v7()),
            provider_name: "OpenAI".to_owned(),
        };
        let connection = || ConnectionActivityParams {
            connection_id: ConnectionId::from_uuid(Uuid::now_v7()),
            connection_name: "Prod S3".to_owned(),
        };
        vec![
            ActivityPayload::ProviderCreated(provider()),
            ActivityPayload::ProviderUpdated(provider()),
            ActivityPayload::ProviderDeleted(provider()),
            ActivityPayload::ConnectionCreated(connection()),
            ActivityPayload::MemberAdded(MemberActivityParams {
                member_username: Handle::from_str("alice").unwrap(),
            }),
        ]
    }

    #[test]
    fn activity_type_tag_matches_the_serde_tag() {
        // The hand-written payload -> ActivityType map must agree with the serde
        // `type` tag the payload serializes under, or a row's stored type would
        // disagree with its body.
        for payload in sample_payloads() {
            let serde_tag = serde_json::to_value(&payload).unwrap()["type"]
                .as_str()
                .unwrap()
                .to_owned();
            assert_eq!(payload.activity_type().as_tag(), serde_tag, "{payload:?}");
        }
    }

    #[test]
    fn provider_events_map_to_the_matching_webhook_event() {
        use crate::types::WebhookEvent;
        let cases = [
            (
                ActivityPayload::ProviderCreated(ProviderActivityParams {
                    provider_id: ProviderId::from_uuid(Uuid::now_v7()),
                    provider_name: "x".to_owned(),
                }),
                WebhookEvent::ProviderCreated,
            ),
            (
                ActivityPayload::ProviderDeleted(ProviderActivityParams {
                    provider_id: ProviderId::from_uuid(Uuid::now_v7()),
                    provider_name: "x".to_owned(),
                }),
                WebhookEvent::ProviderDeleted,
            ),
        ];
        for (payload, expected) in cases {
            assert_eq!(payload.webhook_event(), Some(expected), "{payload:?}");
        }
    }

    #[test]
    fn provider_object_has_prefixed_id_and_name_label() {
        let provider_id = ProviderId::from_uuid(Uuid::now_v7());
        let p = ActivityPayload::ProviderUpdated(ProviderActivityParams {
            provider_id,
            provider_name: "OpenAI GPT-4".to_owned(),
        });
        assert_eq!(p.object_id(), Some(provider_id.to_string()));
        assert!(p.object_id().unwrap().starts_with("prov_"));
        assert_eq!(p.object_label(), Some("OpenAI GPT-4".to_owned()));
    }

    #[test]
    fn workspace_events_carry_no_webhook() {
        // The workspace lifecycle is not a webhook event; `webhook_event` returns
        // `None` so the drainer does not emit one.
        let p = ActivityPayload::WorkspaceCreated(WorkspaceActivityParams {
            workspace_slug: Handle::from_str("acme").unwrap(),
        });
        assert_eq!(p.webhook_event(), None);
    }

    #[test]
    fn object_with_both_id_and_label() {
        let id = Uuid::now_v7();
        let p = ActivityPayload::InviteCreated(InviteActivityParams {
            invite_id: id,
            email: Some("a@b.com".to_owned()),
        });
        assert_eq!(p.object_id(), Some(id.to_string()));
        assert_eq!(p.object_label(), Some("a@b.com".to_owned()));
    }

    #[test]
    fn serializes_as_a_type_data_envelope_and_round_trips() {
        let file_id = Uuid::now_v7();
        let payload = ActivityPayload::FileCreated(FileActivityParams {
            file_id,
            file_name: "report.pdf".to_owned(),
        });

        let value = serde_json::to_value(&payload).unwrap();
        // The durable wire shape: a `type` tag and a nested `data` object (the same
        // envelope the notification payload and outbox event use). The stored rows
        // depend on this, so pin it.
        assert_eq!(value["type"], "file.created");
        assert_eq!(value["data"]["fileId"], file_id.to_string());
        assert_eq!(value["data"]["fileName"], "report.pdf");
        assert!(
            value.get("fileId").is_none(),
            "params must nest under `data`"
        );

        let decoded: ActivityPayload = serde_json::from_value(value).unwrap();
        assert!(matches!(decoded, ActivityPayload::FileCreated(_)));
    }
}
