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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkspaceEvent {
    // Workspace
    WorkspaceCreated {
        workspace_slug: Handle,
    },
    WorkspaceUpdated {
        workspace_slug: Handle,
    },
    WorkspaceDeleted {
        workspace_id: Uuid,
        workspace_slug: Handle,
    },

    // Members
    MemberAdded {
        member_username: Handle,
    },
    MemberUpdated(MemberRef),
    MemberDeleted(MemberRef),

    // Invites
    InviteCreated(InviteRef),
    InviteAccepted(InviteRef),
    InviteDeclined(InviteRef),
    InviteCanceled(InviteRef),

    // Connections
    ConnectionCreated(ConnectionRef),
    ConnectionUpdated(ConnectionRef),
    ConnectionDeleted(ConnectionRef),
    ConnectionSyncCompleted {
        connection_id: Uuid,
        records_synced: Option<i64>,
        notify: Option<Uuid>,
    },
    ConnectionSyncFailed {
        connection_id: Uuid,
        error: Option<String>,
        notify: Option<Uuid>,
    },

    // Webhooks
    WebhookCreated(WebhookRef),
    WebhookUpdated(WebhookRef),
    WebhookDeleted(WebhookRef),

    // Files
    FileCreated {
        file: FileRef,
        file_size_bytes: i64,
    },
    FileUpdated(FileRef),
    FileDeleted(FileRef),

    // Pipelines
    PipelineCreated(PipelineRef),
    PipelineUpdated(PipelineRef),
    PipelineDeleted(PipelineRef),

    // Pipeline runs
    PipelineRunStarted(PipelineRunRef),
    PipelineRunAnalyzed {
        run: PipelineRunRef,
        input_file_name: Option<String>,
        notify: Uuid,
    },
    PipelineRunCompleted {
        run: PipelineRunRef,
        input_file_name: Option<String>,
        notify: Uuid,
    },
    PipelineRunFailed {
        run: PipelineRunRef,
        input_file_name: Option<String>,
        error: Option<String>,
        notify: Uuid,
    },

    // Policies
    PolicyCreated(PolicyRef),
    PolicyUpdated(PolicyRef),
    PolicyDeleted(PolicyRef),
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
    pub email: String,
}

/// A connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionRef {
    pub connection_id: Uuid,
}

/// A webhook.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookRef {
    pub webhook_id: Uuid,
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

/// A policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyRef {
    pub policy_id: Uuid,
}
