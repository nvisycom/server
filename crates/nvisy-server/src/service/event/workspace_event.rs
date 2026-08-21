//! The workspace-event vocabulary: what happened, as raw domain facts.
//!
//! A [`WorkspaceEvent`] carries only the facts of an action — no knowledge of the
//! sinks it feeds. The drainer projects each event onto the activity log, the
//! webhook stream, and notifications. Facts are owned and serializable so an
//! event is persisted to the outbox and drained later.
//!
//! Variants that share a field-set carry it as one of the small `*Ref` structs
//! below the enum, so the shape is written once and the variants stay uniform.

use nvisy_postgres::types::Handle;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A workspace event, as the raw domain facts.
///
/// The wire format is pinned: variants are tagged by an explicit, stable `type`
/// string (not the Rust identifier), and every variant's body is a single `*Ref`
/// payload under `data`. An outbox row written by one build is decoded by a
/// later one, so a Rust-side rename must never change the stored JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum WorkspaceEvent {
    // Workspace
    #[serde(rename = "workspace.created")]
    WorkspaceCreated(WorkspaceRef),
    #[serde(rename = "workspace.updated")]
    WorkspaceUpdated(WorkspaceRef),
    #[serde(rename = "workspace.deleted")]
    WorkspaceDeleted(WorkspaceRef),

    // Members
    #[serde(rename = "member.added")]
    MemberAdded(MemberRef),
    #[serde(rename = "member.updated")]
    MemberUpdated(MemberRef),
    #[serde(rename = "member.deleted")]
    MemberDeleted(MemberRef),

    // Invites
    #[serde(rename = "invite.created")]
    InviteCreated(InviteRef),
    #[serde(rename = "invite.accepted")]
    InviteAccepted(InviteRef),
    #[serde(rename = "invite.declined")]
    InviteDeclined(InviteRef),
    #[serde(rename = "invite.canceled")]
    InviteCanceled(InviteRef),

    // Connections
    #[serde(rename = "connection.created")]
    ConnectionCreated(ConnectionRef),
    #[serde(rename = "connection.updated")]
    ConnectionUpdated(ConnectionRef),
    #[serde(rename = "connection.deleted")]
    ConnectionDeleted(ConnectionRef),
    #[serde(rename = "connection.sync.started")]
    ConnectionSyncStarted(ConnectionRef),
    #[serde(rename = "connection.sync.completed")]
    ConnectionSyncCompleted {
        connection_id: Uuid,
        connection_name: String,
        records_synced: Option<i64>,
        notify: Option<Uuid>,
    },
    #[serde(rename = "connection.sync.failed")]
    ConnectionSyncFailed {
        connection_id: Uuid,
        connection_name: String,
        error: Option<String>,
        notify: Option<Uuid>,
    },

    // Webhooks
    #[serde(rename = "webhook.created")]
    WebhookCreated(WebhookRef),
    #[serde(rename = "webhook.updated")]
    WebhookUpdated(WebhookRef),
    #[serde(rename = "webhook.deleted")]
    WebhookDeleted(WebhookRef),

    // Files
    #[serde(rename = "file.created")]
    FileCreated {
        #[serde(flatten)]
        file: FileRef,
        file_size_bytes: i64,
    },
    #[serde(rename = "file.updated")]
    FileUpdated(FileRef),
    #[serde(rename = "file.deleted")]
    FileDeleted(FileRef),

    // Pipelines
    #[serde(rename = "pipeline.created")]
    PipelineCreated(PipelineRef),
    #[serde(rename = "pipeline.updated")]
    PipelineUpdated(PipelineRef),
    #[serde(rename = "pipeline.deleted")]
    PipelineDeleted(PipelineRef),

    // Pipeline runs
    #[serde(rename = "pipeline.run.started")]
    PipelineRunStarted(PipelineRunRef),
    #[serde(rename = "pipeline.run.analyzed")]
    PipelineRunAnalyzed {
        #[serde(flatten)]
        run: PipelineRunRef,
        input_file_name: Option<String>,
        notify: Uuid,
    },
    #[serde(rename = "pipeline.run.completed")]
    PipelineRunCompleted {
        #[serde(flatten)]
        run: PipelineRunRef,
        input_file_name: Option<String>,
        notify: Uuid,
    },
    #[serde(rename = "pipeline.run.failed")]
    PipelineRunFailed {
        #[serde(flatten)]
        run: PipelineRunRef,
        input_file_name: Option<String>,
        error: Option<String>,
        notify: Uuid,
    },

    // Policies
    #[serde(rename = "policy.created")]
    PolicyCreated(PolicyRef),
    #[serde(rename = "policy.updated")]
    PolicyUpdated(PolicyRef),
    #[serde(rename = "policy.deleted")]
    PolicyDeleted(PolicyRef),
}

/// A workspace and its slug.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceRef {
    pub workspace_id: Uuid,
    pub workspace_slug: Handle,
}

/// A member and their username.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberRef {
    pub member_id: Uuid,
    pub member_username: Handle,
}

/// An invitation and the address it was sent to.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InviteRef {
    pub invite_id: Uuid,
    /// The invitee's email, when the invitation recorded one. `None` when the
    /// invite carried no address, so an absent address stays distinct from a
    /// blank one.
    pub email: Option<String>,
}

/// A connection and its display name.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionRef {
    pub connection_id: Uuid,
    pub connection_name: String,
}

/// A webhook and its display name.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookRef {
    pub webhook_id: Uuid,
    pub webhook_name: String,
}

/// A file and its display name.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRef {
    pub file_id: Uuid,
    pub file_name: String,
}

/// A pipeline and its slug.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineRef {
    pub pipeline_id: Uuid,
    pub pipeline_slug: Handle,
}

/// A pipeline run and its pipeline's slug.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineRunRef {
    pub run_id: Uuid,
    pub pipeline_slug: Handle,
}

/// A policy and its slug.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyRef {
    pub policy_id: Uuid,
    pub policy_slug: Handle,
}
