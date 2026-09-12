//! The workspace-event vocabulary: one struct per event, each the single source
//! of truth for its facts and its projection onto the three sinks.
//!
//! Each event is a plain, serializable struct that owns its fields once and
//! implements [`EventKind`]: the trait builds the activity, webhook, and
//! notification payloads from those fields, so a field set is declared a single
//! time rather than re-typed per sink. The [`workspace_events!`] macro binds the
//! structs into the [`WorkspaceEvent`] outbox envelope and its dispatch.
//!
//! The wire format is pinned: an outbox row written by one build is decoded by a
//! later one, so a variant's tag and an event struct's field names must not
//! change.

use nvisy_postgres::types::{
    ActivityPayload, CommentMentionedParams, ConnectionActivityParams, ConnectionId,
    ConnectionSyncCompletedParams, ConnectionSyncFailedParams, DetectionActivityParams,
    DetectionCompletedParams, DetectionFailedParams, DetectionId, DocumentActivityParams, Handle,
    InviteActivityParams, MemberActivityParams, MemberJoinedParams, NotificationPayload,
    PipelineActivityParams, PolicyActivityParams, ProviderActivityParams, ProviderId,
    RedactionActivityParams, RedactionCreatedParams, RedactionId, ReviewActivityParams,
    ReviewAssignedParams, ThreadActivityParams, ThreadCommentActivityParams, WebhookActivityParams,
    WebhookEvent, WebhookId, WorkspaceActivityParams, WorkspaceRole,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::service::event::kind::{EventKind, Notification, NotifyTarget, WebhookDelivery};
use crate::service::event::macros::{crud_events, workspace_events};

workspace_events! {
    WorkspaceCreated        => "workspace.created",
    WorkspaceUpdated        => "workspace.updated",
    WorkspaceDeleted        => "workspace.deleted",

    MemberAdded             => "member.added",
    MemberUpdated           => "member.updated",
    MemberDeleted           => "member.deleted",

    InviteCreated           => "invite.created",
    InviteAccepted          => "invite.accepted",
    InviteDeclined          => "invite.declined",
    InviteCanceled          => "invite.canceled",

    ConnectionCreated       => "connection.created",
    ConnectionUpdated       => "connection.updated",
    ConnectionDeleted       => "connection.deleted",
    ConnectionSyncStarted   => "connection.sync.started",
    ConnectionSyncCompleted => "connection.sync.completed",
    ConnectionSyncFailed    => "connection.sync.failed",

    ProviderCreated         => "provider.created",
    ProviderUpdated         => "provider.updated",
    ProviderDeleted         => "provider.deleted",

    WebhookCreated          => "webhook.created",
    WebhookUpdated          => "webhook.updated",
    WebhookDeleted          => "webhook.deleted",

    DocumentCreated         => "document.created",
    DocumentUpdated         => "document.updated",
    DocumentDeleted         => "document.deleted",

    ReviewVerified          => "review.verified",
    ReviewAssigned          => "review.assigned",
    ReviewUnassigned        => "review.unassigned",

    PipelineCreated         => "pipeline.created",
    PipelineUpdated         => "pipeline.updated",
    PipelineDeleted         => "pipeline.deleted",

    DetectionStarted        => "pipeline.detection.started",
    DetectionCompleted      => "pipeline.detection.completed",
    DetectionFailed         => "pipeline.detection.failed",

    RedactionCreated        => "pipeline.redaction.created",

    PolicyCreated           => "policy.created",
    PolicyUpdated           => "policy.updated",
    PolicyDeleted           => "policy.deleted",

    ThreadOpened         => "thread.opened",
    ThreadClosed         => "thread.closed",
    ThreadReopened       => "thread.reopened",
    ThreadRenamed        => "thread.renamed",
    ThreadDeleted        => "thread.deleted",
    ThreadCommentCreated => "thread.comment.created",
}

/// The webhook body for a document event: just the document's display name.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DocumentWebhookBody<'a> {
    display_name: &'a str,
}

/// The webhook body for `document.created`: the display name plus the byte size.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DocumentCreatedWebhookBody<'a> {
    display_name: &'a str,
    document_size_bytes: i64,
}

/// The webhook body for a review event: the document name, plus the assignee when the
/// review is assigned (omitted for verification or when clearing the assignee).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReviewWebhookBody<'a> {
    display_name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    assignee: Option<&'a Handle>,
}

/// Serializes a webhook body to JSON, failing closed to `None` (no body) rather
/// than dropping the whole delivery if serialization ever fails.
fn webhook_body<T: Serialize>(body: &T) -> Option<serde_json::Value> {
    serde_json::to_value(body).ok()
}

// Workspace lifecycle events.
crud_events! {
    fields { workspace_id: Uuid, workspace_slug: Handle }
    id = workspace_id;
    activity(this) = WorkspaceActivityParams { workspace_slug: this.workspace_slug.clone() };
    webhook = no;

    /// A workspace was created.
    WorkspaceCreated => "workspace.created",
    /// A workspace was updated.
    WorkspaceUpdated => "workspace.updated",
    /// A workspace was deleted.
    WorkspaceDeleted => "workspace.deleted",
}

/// A member was added / updated / removed.
///
/// `MemberAdded` also raises the `member.joined` in-app notification to the
/// workspace's owners and admins (excluding the joiner), so it carries the
/// workspace slug and the joiner's account id the notification needs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberAdded {
    pub member_id: Uuid,
    pub member_username: Handle,
    pub workspace_slug: Handle,
}

impl EventKind for MemberAdded {
    const TAG: &'static str = "member.added";

    fn resource_id(&self) -> Uuid {
        self.member_id
    }

    fn activity(&self) -> ActivityPayload {
        ActivityPayload::MemberAdded(MemberActivityParams {
            member_username: self.member_username.clone(),
        })
    }

    fn webhook(&self) -> Option<WebhookDelivery> {
        Some(WebhookDelivery {
            event: WebhookEvent::MemberAdded,
            body: None,
        })
    }

    fn notification(self) -> Vec<Notification> {
        // Tell the workspace's owners and admins that someone joined, skipping the
        // joiner themselves.
        vec![Notification {
            target: NotifyTarget::Roles {
                roles: vec![WorkspaceRole::Owner, WorkspaceRole::Admin],
                exclude: Some(self.member_id),
            },
            payload: NotificationPayload::MemberJoined(MemberJoinedParams {
                workspace_slug: self.workspace_slug,
                member_username: self.member_username,
            }),
        }]
    }
}

// Member update/removal. (MemberAdded is hand-written above: it also raises the
// member.joined notification and carries the fields that needs.)
crud_events! {
    fields { member_id: Uuid, member_username: Handle }
    id = member_id;
    activity(this) = MemberActivityParams { member_username: this.member_username.clone() };
    webhook = yes;

    /// A member's role or notification preferences were updated.
    MemberUpdated => "member.updated",
    /// A member was removed from the workspace.
    MemberDeleted => "member.deleted",
}

// Invite lifecycle events. The invitee's email is recorded when the invite
// carried one; `None` keeps an absent address distinct from a blank one. No
// webhook — the invite flow is internal to the workspace.
crud_events! {
    fields { invite_id: Uuid, email: Option<String> }
    id = invite_id;
    activity(this) = InviteActivityParams { invite_id: this.invite_id, email: this.email.clone() };
    webhook = no;

    /// An invitation was created.
    InviteCreated => "invite.created",
    /// An invitation was accepted.
    InviteAccepted => "invite.accepted",
    /// An invitation was declined.
    InviteDeclined => "invite.declined",
    /// An invitation was canceled.
    InviteCanceled => "invite.canceled",
}

// Connection lifecycle + sync-started. (The sync completed/failed events are
// hand-written below: they notify the triggering account and carry extra fields.)
crud_events! {
    fields { connection_id: Uuid, connection_name: String }
    id = connection_id;
    activity(this) = connection_activity(this.connection_id, &this.connection_name);
    webhook = yes;

    /// A connection was created.
    ConnectionCreated => "connection.created",
    /// A connection was updated.
    ConnectionUpdated => "connection.updated",
    /// A connection was deleted.
    ConnectionDeleted => "connection.deleted",
    /// A connection's sync started.
    ConnectionSyncStarted => "connection.sync.started",
}

/// A connection sync completed. Notifies the triggering account, when known.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionSyncCompleted {
    pub connection_id: Uuid,
    pub connection_name: String,
    pub records_synced: Option<i64>,
    pub notify: Option<Uuid>,
}

impl EventKind for ConnectionSyncCompleted {
    const TAG: &'static str = "connection.sync.completed";

    fn resource_id(&self) -> Uuid {
        self.connection_id
    }

    fn activity(&self) -> ActivityPayload {
        ActivityPayload::ConnectionSyncCompleted(connection_activity(
            self.connection_id,
            &self.connection_name,
        ))
    }

    fn webhook(&self) -> Option<WebhookDelivery> {
        Some(WebhookDelivery {
            event: WebhookEvent::ConnectionSyncCompleted,
            body: None,
        })
    }

    fn notification(self) -> Vec<Notification> {
        Notification::to_account(
            self.notify,
            NotificationPayload::ConnectionSyncCompleted(ConnectionSyncCompletedParams {
                connection_id: ConnectionId::from_uuid(self.connection_id),
                connection_name: self.connection_name,
                records_synced: self.records_synced,
            }),
        )
    }
}

/// A connection sync failed. Notifies the triggering account, when known.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionSyncFailed {
    pub connection_id: Uuid,
    pub connection_name: String,
    pub error: Option<String>,
    pub notify: Option<Uuid>,
}

impl EventKind for ConnectionSyncFailed {
    const TAG: &'static str = "connection.sync.failed";

    fn resource_id(&self) -> Uuid {
        self.connection_id
    }

    fn activity(&self) -> ActivityPayload {
        ActivityPayload::ConnectionSyncFailed(connection_activity(
            self.connection_id,
            &self.connection_name,
        ))
    }

    fn webhook(&self) -> Option<WebhookDelivery> {
        Some(WebhookDelivery {
            event: WebhookEvent::ConnectionSyncFailed,
            body: None,
        })
    }

    fn notification(self) -> Vec<Notification> {
        Notification::to_account(
            self.notify,
            NotificationPayload::ConnectionSyncFailed(ConnectionSyncFailedParams {
                connection_id: ConnectionId::from_uuid(self.connection_id),
                connection_name: self.connection_name,
                error: self.error,
            }),
        )
    }
}

/// Builds the shared connection activity params.
fn connection_activity(connection_id: Uuid, connection_name: &str) -> ConnectionActivityParams {
    ConnectionActivityParams {
        connection_id: ConnectionId::from_uuid(connection_id),
        connection_name: connection_name.to_owned(),
    }
}

// Provider lifecycle events.
crud_events! {
    fields { provider_id: Uuid, provider_name: String }
    id = provider_id;
    activity(this) = provider_activity(this.provider_id, &this.provider_name);
    webhook = yes;

    /// A provider was created.
    ProviderCreated => "provider.created",
    /// A provider was updated.
    ProviderUpdated => "provider.updated",
    /// A provider was deleted.
    ProviderDeleted => "provider.deleted",
}

/// Builds the shared provider activity params.
fn provider_activity(provider_id: Uuid, provider_name: &str) -> ProviderActivityParams {
    ProviderActivityParams {
        provider_id: ProviderId::from_uuid(provider_id),
        provider_name: provider_name.to_owned(),
    }
}

// Webhook lifecycle events. Webhook CRUD does not itself fire a webhook (it is
// recorded only in the activity log).
crud_events! {
    fields { webhook_id: Uuid, webhook_name: String }
    id = webhook_id;
    activity(this) = webhook_activity(this.webhook_id, &this.webhook_name);
    webhook = no;

    /// A webhook was created.
    WebhookCreated => "webhook.created",
    /// A webhook was updated.
    WebhookUpdated => "webhook.updated",
    /// A webhook was deleted.
    WebhookDeleted => "webhook.deleted",
}

/// Builds the shared webhook activity params.
fn webhook_activity(webhook_id: Uuid, webhook_name: &str) -> WebhookActivityParams {
    WebhookActivityParams {
        webhook_id: WebhookId::from_uuid(webhook_id),
        webhook_name: webhook_name.to_owned(),
    }
}

/// A document was created. Carries its byte size for the webhook body.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentCreated {
    pub document_id: Uuid,
    pub document_name: String,
    pub document_size_bytes: i64,
}

impl EventKind for DocumentCreated {
    const TAG: &'static str = "document.created";

    fn resource_id(&self) -> Uuid {
        self.document_id
    }

    fn activity(&self) -> ActivityPayload {
        ActivityPayload::DocumentCreated(document_activity(self.document_id, &self.document_name))
    }

    fn webhook(&self) -> Option<WebhookDelivery> {
        Some(WebhookDelivery {
            event: WebhookEvent::DocumentCreated,
            body: webhook_body(&DocumentCreatedWebhookBody {
                display_name: &self.document_name,
                document_size_bytes: self.document_size_bytes,
            }),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentUpdated {
    pub document_id: Uuid,
    pub document_name: String,
}

impl EventKind for DocumentUpdated {
    const TAG: &'static str = "document.updated";

    fn resource_id(&self) -> Uuid {
        self.document_id
    }

    fn activity(&self) -> ActivityPayload {
        ActivityPayload::DocumentUpdated(document_activity(self.document_id, &self.document_name))
    }

    fn webhook(&self) -> Option<WebhookDelivery> {
        Some(WebhookDelivery {
            event: WebhookEvent::DocumentUpdated,
            body: webhook_body(&DocumentWebhookBody {
                display_name: &self.document_name,
            }),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentDeleted {
    pub document_id: Uuid,
    pub document_name: String,
}

impl EventKind for DocumentDeleted {
    const TAG: &'static str = "document.deleted";

    fn resource_id(&self) -> Uuid {
        self.document_id
    }

    fn activity(&self) -> ActivityPayload {
        ActivityPayload::DocumentDeleted(document_activity(self.document_id, &self.document_name))
    }

    fn webhook(&self) -> Option<WebhookDelivery> {
        Some(WebhookDelivery {
            event: WebhookEvent::DocumentDeleted,
            body: webhook_body(&DocumentWebhookBody {
                display_name: &self.document_name,
            }),
        })
    }
}

/// Builds the shared document activity params.
fn document_activity(document_id: Uuid, document_name: &str) -> DocumentActivityParams {
    DocumentActivityParams {
        document_id,
        document_name: document_name.to_owned(),
    }
}

/// A document's review was verified (the review thread reached `resolved`). Raises no
/// notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewVerified {
    pub thread_id: Uuid,
    pub document_id: Uuid,
    pub document_name: String,
}

impl EventKind for ReviewVerified {
    const TAG: &'static str = "review.verified";

    fn resource_id(&self) -> Uuid {
        self.thread_id
    }

    fn activity(&self) -> ActivityPayload {
        ActivityPayload::ReviewVerified(ReviewActivityParams {
            thread_id: self.thread_id,
            document_id: self.document_id,
            assignee_username: None,
        })
    }

    fn webhook(&self) -> Option<WebhookDelivery> {
        Some(WebhookDelivery {
            event: WebhookEvent::ReviewVerified,
            body: webhook_body(&ReviewWebhookBody {
                display_name: &self.document_name,
                assignee: None,
            }),
        })
    }
}

/// A document's review was assigned to a reviewer. A document always exists at
/// assign time, so its name is present. Notifies the reviewer unless they assigned
/// themselves.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewAssigned {
    pub thread_id: Uuid,
    pub document_id: Uuid,
    pub document_name: String,
    pub assignee_username: Handle,
    pub notify: Option<Uuid>,
}

impl EventKind for ReviewAssigned {
    const TAG: &'static str = "review.assigned";

    fn resource_id(&self) -> Uuid {
        self.thread_id
    }

    fn activity(&self) -> ActivityPayload {
        ActivityPayload::ReviewAssigned(ReviewActivityParams {
            thread_id: self.thread_id,
            document_id: self.document_id,
            assignee_username: Some(self.assignee_username.clone()),
        })
    }

    fn webhook(&self) -> Option<WebhookDelivery> {
        Some(WebhookDelivery {
            event: WebhookEvent::ReviewAssigned,
            body: webhook_body(&ReviewWebhookBody {
                display_name: &self.document_name,
                assignee: Some(&self.assignee_username),
            }),
        })
    }

    fn notification(self) -> Vec<Notification> {
        Notification::to_account(
            self.notify,
            NotificationPayload::ReviewAssigned(ReviewAssignedParams {
                thread_id: self.thread_id,
                document_id: self.document_id,
                document_name: Some(self.document_name),
            }),
        )
    }
}

/// A document's review assignee was cleared. The document may since have been
/// removed, so its name is optional. Raises no notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewUnassigned {
    pub thread_id: Uuid,
    pub document_id: Uuid,
    pub document_name: Option<String>,
}

impl EventKind for ReviewUnassigned {
    const TAG: &'static str = "review.unassigned";

    fn resource_id(&self) -> Uuid {
        self.thread_id
    }

    fn activity(&self) -> ActivityPayload {
        ActivityPayload::ReviewUnassigned(ReviewActivityParams {
            thread_id: self.thread_id,
            document_id: self.document_id,
            assignee_username: None,
        })
    }

    fn webhook(&self) -> Option<WebhookDelivery> {
        Some(WebhookDelivery {
            event: WebhookEvent::ReviewUnassigned,
            body: webhook_body(&ReviewWebhookBody {
                display_name: self.document_name.as_deref().unwrap_or_default(),
                assignee: None,
            }),
        })
    }
}

// Pipeline lifecycle events. (Detection/redaction run events are hand-written
// below: they carry run-specific fields and notifications.)
crud_events! {
    fields { pipeline_id: Uuid, pipeline_slug: Handle }
    id = pipeline_id;
    activity(this) = PipelineActivityParams { pipeline_slug: this.pipeline_slug.clone() };
    webhook = yes;

    /// A pipeline was created.
    PipelineCreated => "pipeline.created",
    /// A pipeline was updated.
    PipelineUpdated => "pipeline.updated",
    /// A pipeline was deleted.
    PipelineDeleted => "pipeline.deleted",
}

// A detection started: a plain activity + webhook event, no notification. (The
// completed/failed events below notify the triggering account.)
crud_events! {
    fields { detection_id: Uuid, pipeline_slug: Handle }
    id = detection_id;
    activity(this) = detection_activity(this.detection_id, &this.pipeline_slug);
    webhook = yes;

    /// A detection was started.
    DetectionStarted => "pipeline.detection.started",
}

/// A detection finished analysis. Notifies the triggering account.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionCompleted {
    pub detection_id: Uuid,
    pub pipeline_slug: Handle,
    pub input_document_name: Option<String>,
    pub notify: Uuid,
}

impl EventKind for DetectionCompleted {
    const TAG: &'static str = "pipeline.detection.completed";

    fn resource_id(&self) -> Uuid {
        self.detection_id
    }

    fn activity(&self) -> ActivityPayload {
        ActivityPayload::DetectionCompleted(detection_activity(
            self.detection_id,
            &self.pipeline_slug,
        ))
    }

    fn webhook(&self) -> Option<WebhookDelivery> {
        Some(WebhookDelivery {
            event: WebhookEvent::DetectionCompleted,
            body: None,
        })
    }

    fn notification(self) -> Vec<Notification> {
        vec![Notification {
            target: NotifyTarget::Account(self.notify),
            payload: NotificationPayload::DetectionCompleted(DetectionCompletedParams {
                detection_id: DetectionId::from_uuid(self.detection_id),
                pipeline_slug: self.pipeline_slug,
                input_document_name: self.input_document_name,
            }),
        }]
    }
}

/// A detection failed. Notifies the triggering account.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionFailed {
    pub detection_id: Uuid,
    pub pipeline_slug: Handle,
    pub input_document_name: Option<String>,
    pub error: Option<String>,
    pub notify: Uuid,
}

impl EventKind for DetectionFailed {
    const TAG: &'static str = "pipeline.detection.failed";

    fn resource_id(&self) -> Uuid {
        self.detection_id
    }

    fn activity(&self) -> ActivityPayload {
        ActivityPayload::DetectionFailed(detection_activity(self.detection_id, &self.pipeline_slug))
    }

    fn webhook(&self) -> Option<WebhookDelivery> {
        Some(WebhookDelivery {
            event: WebhookEvent::DetectionFailed,
            body: None,
        })
    }

    fn notification(self) -> Vec<Notification> {
        vec![Notification {
            target: NotifyTarget::Account(self.notify),
            payload: NotificationPayload::DetectionFailed(DetectionFailedParams {
                detection_id: DetectionId::from_uuid(self.detection_id),
                pipeline_slug: self.pipeline_slug,
                input_document_name: self.input_document_name,
                error: self.error,
            }),
        }]
    }
}

/// Builds the shared detection activity params.
fn detection_activity(detection_id: Uuid, pipeline_slug: &Handle) -> DetectionActivityParams {
    DetectionActivityParams {
        pipeline_slug: pipeline_slug.clone(),
        detection_id: DetectionId::from_uuid(detection_id),
    }
}

/// A redaction was created from a detection. Notifies the triggering account.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedactionCreated {
    pub detection_id: Uuid,
    pub pipeline_slug: Handle,
    /// The redaction that was produced (its own id, distinct from the detection's
    /// — a detection can produce many redactions).
    pub redaction_id: Uuid,
    pub input_document_name: Option<String>,
    pub notify: Uuid,
}

impl EventKind for RedactionCreated {
    const TAG: &'static str = "pipeline.redaction.created";

    fn resource_id(&self) -> Uuid {
        // The affected resource is the redaction produced, not the detection it
        // came from, this matches the activity log's object id for the event.
        self.redaction_id
    }

    fn activity(&self) -> ActivityPayload {
        ActivityPayload::RedactionCreated(RedactionActivityParams {
            pipeline_slug: self.pipeline_slug.clone(),
            redaction_id: RedactionId::from_uuid(self.redaction_id),
        })
    }

    fn webhook(&self) -> Option<WebhookDelivery> {
        Some(WebhookDelivery {
            event: WebhookEvent::RedactionCreated,
            body: None,
        })
    }

    fn notification(self) -> Vec<Notification> {
        vec![Notification {
            target: NotifyTarget::Account(self.notify),
            payload: NotificationPayload::RedactionCreated(RedactionCreatedParams {
                redaction_id: RedactionId::from_uuid(self.redaction_id),
                detection_id: DetectionId::from_uuid(self.detection_id),
                pipeline_slug: self.pipeline_slug,
                input_document_name: self.input_document_name,
            }),
        }]
    }
}

// Policy lifecycle events.
crud_events! {
    fields { policy_id: Uuid, policy_slug: Handle }
    id = policy_id;
    activity(this) = policy_activity(this.policy_id, &this.policy_slug);
    webhook = yes;

    /// A policy was created.
    PolicyCreated => "policy.created",
    /// A policy was updated.
    PolicyUpdated => "policy.updated",
    /// A policy was deleted.
    PolicyDeleted => "policy.deleted",
}

/// Builds the shared policy activity params.
fn policy_activity(policy_id: Uuid, policy_slug: &Handle) -> PolicyActivityParams {
    PolicyActivityParams {
        policy_id,
        policy_slug: policy_slug.clone(),
    }
}

/// A thread was opened with its first message. Feeds activity + webhook,
/// and notifies each account mentioned in the opening body (never the author,
/// even if they @-mention themselves).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadOpened {
    pub thread_id: Uuid,
    /// Id of the thread's opening comment, referenced by mention notifications.
    pub opening_comment_id: Uuid,
    pub document_id: Option<Uuid>,
    /// Username of the thread's opener, shown in the mention notification.
    pub author_username: Handle,
    /// Accounts mentioned in the opening body, to notify. Empty when none.
    pub mentioned: Vec<Uuid>,
}

impl EventKind for ThreadOpened {
    const TAG: &'static str = "thread.opened";

    fn resource_id(&self) -> Uuid {
        self.thread_id
    }

    fn activity(&self) -> ActivityPayload {
        ActivityPayload::ThreadOpened(ThreadActivityParams {
            thread_id: self.thread_id,
            document_id: self.document_id,
        })
    }

    fn webhook(&self) -> Option<WebhookDelivery> {
        Some(WebhookDelivery {
            event: WebhookEvent::ThreadOpened,
            body: None,
        })
    }

    fn notification(self) -> Vec<Notification> {
        // One "you were mentioned" notification per mentioned account. The
        // opening message is part of the thread; its mentions notify here.
        self.mentioned
            .into_iter()
            .map(|recipient| Notification {
                target: NotifyTarget::Account(recipient),
                payload: NotificationPayload::CommentMentioned(CommentMentionedParams {
                    comment_id: self.opening_comment_id,
                    thread_id: self.thread_id,
                    document_id: self.document_id,
                    author_username: self.author_username.clone(),
                }),
            })
            .collect()
    }
}

// Thread close / reopen / rename: activity + webhook, no notification or extra
// fields.
crud_events! {
    fields { thread_id: Uuid, document_id: Option<Uuid> }
    id = thread_id;
    activity(this) = ThreadActivityParams { thread_id: this.thread_id, document_id: this.document_id };
    webhook = yes;

    /// A thread was closed.
    ThreadClosed => "thread.closed",
    /// A thread was reopened.
    ThreadReopened => "thread.reopened",
    /// A thread's title was changed.
    ThreadRenamed => "thread.renamed",
}

// Thread deletion: activity only, no webhook.
crud_events! {
    fields { thread_id: Uuid, document_id: Option<Uuid> }
    id = thread_id;
    activity(this) = ThreadActivityParams { thread_id: this.thread_id, document_id: this.document_id };
    webhook = no;

    /// A thread was deleted.
    ThreadDeleted => "thread.deleted",
}

/// A comment (message) was posted in a thread. Notifies each mentioned account
/// (never the author, even if they @-mention themselves); activity only, no
/// webhook (thread lifecycle carries the webhook signal).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadCommentCreated {
    pub comment_id: Uuid,
    pub thread_id: Uuid,
    pub document_id: Option<Uuid>,
    /// Username of the comment's author, shown in the mention notification.
    pub author_username: Handle,
    /// Accounts mentioned in the comment body, to notify. Empty when none.
    pub mentioned: Vec<Uuid>,
}

impl EventKind for ThreadCommentCreated {
    const TAG: &'static str = "thread.comment.created";

    fn resource_id(&self) -> Uuid {
        self.comment_id
    }

    fn activity(&self) -> ActivityPayload {
        ActivityPayload::ThreadCommentCreated(ThreadCommentActivityParams {
            comment_id: self.comment_id,
            thread_id: self.thread_id,
            document_id: self.document_id,
        })
    }

    fn notification(self) -> Vec<Notification> {
        // One "you were mentioned" notification per mentioned account.
        self.mentioned
            .into_iter()
            .map(|recipient| Notification {
                target: NotifyTarget::Account(recipient),
                payload: NotificationPayload::CommentMentioned(CommentMentionedParams {
                    comment_id: self.comment_id,
                    thread_id: self.thread_id,
                    document_id: self.document_id,
                    author_username: self.author_username.clone(),
                }),
            })
            .collect()
    }
}
