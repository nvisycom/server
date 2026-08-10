//! Contains constraints, enumerations and other custom types.

mod constants;
mod constraint;
mod enums;
mod filtering;
mod handle;
mod pagination;
mod prefixed_id;
mod settings;
mod sorting;
mod utilities;

pub use constants::{
    DEFAULT_RETENTION_DAYS, EXPIRY_WARNING_MINUTES, LONG_LIVED_THRESHOLD_HOURS,
    RECENTLY_SENT_HOURS, RECENTLY_UPLOADED_HOURS,
};
pub use constraint::{
    AccountApiTokenConstraints, AccountConstraints, AccountNotificationConstraints,
    ConstraintCategory, ConstraintViolation, WorkspaceActivitiesConstraints,
    WorkspaceConnectionConstraints, WorkspaceConnectionSyncConstraints, WorkspaceConstraints,
    WorkspaceFileConstraints, WorkspaceInviteConstraints, WorkspaceMemberConstraints,
    WorkspacePipelineConstraints, WorkspacePipelineReferenceConstraints,
    WorkspacePipelineRunConstraints, WorkspacePolicyConstraints, WorkspaceWebhookConstraints,
};
pub use enums::{
    ActivityCategory, ActivityType, ApiTokenType, FileKind, InviteStatus, NotificationEvent,
    PipelineRunStatus, PipelineStatus, PipelineTriggerType, SyncDeletionPolicy, SyncMode,
    SyncStatus, SyncTriggerType, WebhookEvent, WebhookStatus, WorkspaceRole,
};
pub use filtering::{FileFilter, InviteFilter, MemberFilter};
pub use handle::{HANDLE_MAX_LENGTH, HANDLE_MIN_LENGTH, Handle, HandleError};
pub use pagination::{Cursor, CursorPage, CursorPagination, OffsetPage, OffsetPagination};
pub use prefixed_id::{ConnectionId, PrefixedIdError, RunId, WebhookId};
pub use settings::{
    PIPELINE_RETENTION_KEY, Retention, RetentionOverride, RetentionScope, RetentionSettings,
    WorkspaceSettings,
};
pub use sorting::{
    FileSortBy, FileSortField, InviteSortBy, InviteSortField, MemberSortBy, MemberSortField,
    SortBy, SortOrder,
};
pub use utilities::{
    AccountRefRow, HasCreatedAt, HasDeletedAt, HasExpiresAt, HasGeographicContext,
    HasLastActivityAt, HasOwnership, HasSecurityContext, HasUpdatedAt, Tags, WithAccountRef,
};
