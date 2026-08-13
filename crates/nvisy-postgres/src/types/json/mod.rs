//! Value types stored in `JSONB` columns.
//!
//! Everything persisted as `JSONB` lives here: the generic [`Json`] column
//! wrapper and its [`JsonBody`] decode result, the notification and activity
//! payload enums, and the workspace settings / retention value types.

mod activity_params;
mod notification_params;
mod pipeline_metadata;
mod pipeline_run_metadata;
mod retention;
mod typed_body;
mod typed_json;
mod webhook_headers;
mod workspace_metadata;
mod workspace_settings;

pub use activity_params::{
    ActivityPayload, ConnectionActivityParams, FileActivityParams, InviteActivityParams,
    MemberActivityParams, PipelineActivityParams, PipelineRunActivityParams, PolicyActivityParams,
    WebhookActivityParams, WorkspaceActivityParams,
};
pub use notification_params::{
    ConnectionSyncCompletedParams, ConnectionSyncFailedParams, MemberInvitedParams,
    MemberJoinedParams, NotificationPayload, PipelineRunAnalyzedParams, PipelineRunCompletedParams,
    PipelineRunFailedParams, SystemAnnouncementParams, SystemReportParams,
};
pub use pipeline_metadata::{PipelineMetadata, RetentionOverride};
pub use pipeline_run_metadata::RunMetadata;
pub use retention::{Retention, RetentionScope, RetentionSettings};
pub use typed_body::JsonBody;
pub use typed_json::Json;
pub use webhook_headers::{InvalidHeader, WebhookHeaders};
pub use workspace_metadata::WorkspaceMetadata;
pub use workspace_settings::{OcrPolicy, WorkspaceSettings};
