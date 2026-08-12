//! Value types stored in `JSONB` columns.
//!
//! Everything persisted as `JSONB` lives here: the generic [`TypedJson`] column
//! wrapper and its [`TypedBody`] decode result, the notification and activity
//! payload enums, and the workspace settings / retention value types.

mod activity;
mod notification;
mod retention;
mod settings;
mod typed_body;
mod typed_json;

pub use activity::{
    ActivityPayload, ConnectionActivityParams, FileActivityParams, InviteActivityParams,
    MemberActivityParams, PipelineActivityParams, PipelineRunActivityParams, PolicyActivityParams,
    WebhookActivityParams, WorkspaceActivityParams,
};
pub use notification::{
    ConnectionSyncCompletedParams, ConnectionSyncFailedParams, MemberInvitedParams,
    MemberJoinedParams, NotificationPayload, PipelineRunAnalyzedParams, PipelineRunCompletedParams,
    PipelineRunFailedParams, SystemAnnouncementParams, SystemReportParams,
};
pub use retention::{
    PIPELINE_RETENTION_KEY, Retention, RetentionOverride, RetentionScope, RetentionSettings,
};
pub use settings::{OcrPolicy, WorkspaceSettings};
pub use typed_body::TypedBody;
pub use typed_json::TypedJson;
