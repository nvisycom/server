//! Contains constraints, enumerations and other custom types.

mod constants;
mod constraint;
mod enums;
mod filtering;
mod handle;
mod json;
mod pagination;
mod prefixed_id;
mod sorting;
mod utilities;

pub use constants::{DEFAULT_RETENTION_DAYS, RECENTLY_SENT_HOURS};
pub use constraint::{
    AccountApiTokenConstraints, AccountConstraints, AccountNotificationConstraints,
    ChatMessageConstraints, ChatSessionConstraints, ConstraintViolation,
    WorkspaceActivitiesConstraints, WorkspaceConnectionConstraints,
    WorkspaceConnectionSyncConstraints, WorkspaceConstraints, WorkspaceDetectionConstraints,
    WorkspaceFileConstraints, WorkspaceInviteConstraints, WorkspaceMemberConstraints,
    WorkspacePipelineConstraints, WorkspacePipelineReferenceConstraints,
    WorkspacePolicyConstraints, WorkspaceWebhookConstraints,
};
pub use enums::{
    ActivityType, ApiTokenType, ChatRole, DetectionStatus, FileKind, InviteStatus,
    NotificationEvent, OutboxStatus, PipelineStatus, PipelineTriggerType, ProviderType,
    SyncDeletionPolicy, SyncMode, SyncStatus, SyncTriggerType, WebhookEvent, WebhookStatus,
    WorkspaceRole,
};
pub use filtering::{DetectionFilter, FileFilter, InviteFilter, MemberFilter};
pub use handle::{HANDLE_MAX_LENGTH, HANDLE_MIN_LENGTH, Handle, HandleError};
pub use json::{
    ActivityPayload, ConnectionActivityParams, ConnectionSyncCompletedParams,
    ConnectionSyncFailedParams, DetectionActivityParams, DetectionCompletedParams,
    DetectionFailedParams, DetectionMetadata, FileActivityParams, InvalidHeader,
    InviteActivityParams, Json, MemberActivityParams, MemberInvitedParams, MemberJoinedParams,
    NotificationPayload, OcrPolicy, PipelineActivityParams, PipelineMetadata, PolicyActivityParams,
    RedactionActivityParams, RedactionCreatedParams, Retention, RetentionOverride, RetentionScope,
    RetentionSettings, WebhookActivityParams, WebhookHeaders, WorkspaceActivityParams,
    WorkspaceMetadata, WorkspaceSettings,
};
pub use pagination::{Cursor, CursorPage, CursorPagination, OffsetPage, OffsetPagination};
pub use prefixed_id::{ConnectionId, DetectionId, PrefixedIdError, RedactionId, WebhookId};
pub use sorting::{
    FileSortBy, FileSortField, InviteSortBy, InviteSortField, MemberSortBy, MemberSortField,
    SortBy, SortOrder,
};
pub use utilities::{
    AccountRefRow, HasCreatedAt, HasDeletedAt, HasExpiresAt, HasGeographicContext,
    HasLastActivityAt, HasOwnership, HasSecurityContext, HasUpdatedAt, WithAccountRef,
};
