//! Value types stored in `JSONB` columns.
//!
//! Everything persisted as `JSONB` lives here: the generic [`Json`] column
//! wrapper, the notification and activity payload enums, and the workspace
//! settings / retention value types.

mod activity_params;
mod detection_metadata;
mod notification_params;
mod pipeline_metadata;
mod retention;
mod typed_json;
mod webhook_headers;
mod workspace_metadata;
mod workspace_settings;

pub use activity_params::{
    ActivityPayload, ConnectionActivityParams, DetectionActivityParams, FileActivityParams,
    InviteActivityParams, MemberActivityParams, PipelineActivityParams, PolicyActivityParams,
    RedactionActivityParams, WebhookActivityParams, WorkspaceActivityParams,
};
pub use detection_metadata::DetectionMetadata;
pub use notification_params::{
    ConnectionSyncCompletedParams, ConnectionSyncFailedParams, DetectionCompletedParams,
    DetectionFailedParams, MemberInvitedParams, MemberJoinedParams, NotificationPayload,
    RedactionCreatedParams,
};
pub use pipeline_metadata::{PipelineMetadata, RetentionOverride};
pub use retention::{Retention, RetentionScope, RetentionSettings};
pub use typed_json::Json;
pub use webhook_headers::{InvalidHeader, WebhookHeaders};
pub use workspace_metadata::WorkspaceMetadata;
pub use workspace_settings::{OcrPolicy, WorkspaceSettings};
