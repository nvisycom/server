//! Workspace-related constraint violation error handlers.

use nvisy_postgres::types::{
    WorkspaceActivitiesConstraints, WorkspaceConstraints, WorkspaceInviteConstraints,
    WorkspaceMemberConstraints, WorkspaceThreadCommentConstraints, WorkspaceThreadConstraints,
    WorkspaceWebhookConstraints,
};

use super::{Error, ErrorKind};

impl From<WorkspaceConstraints> for Error<'static> {
    fn from(c: WorkspaceConstraints) -> Self {
        let error = match c {
            WorkspaceConstraints::DisplayNameLength => ErrorKind::BadRequest
                .with_message("Workspace name must be between 3 and 32 characters long"),
            WorkspaceConstraints::SlugLength => ErrorKind::BadRequest
                .with_message("Workspace slug must be between 3 and 32 characters long"),
            WorkspaceConstraints::SlugFormat => ErrorKind::BadRequest.with_message(
                "Workspace slug must be lowercase alphanumeric with single internal dashes",
            ),
            WorkspaceConstraints::NameUnique => {
                ErrorKind::Conflict.with_message("A workspace with this name already exists")
            }
            WorkspaceConstraints::SlugUnique => {
                ErrorKind::Conflict.with_message("A workspace with this slug already exists")
            }
            WorkspaceConstraints::DescriptionLengthMax => {
                ErrorKind::BadRequest.with_message("Workspace description is too long")
            }
            WorkspaceConstraints::MetadataSize => {
                ErrorKind::BadRequest.with_message("Workspace metadata size is invalid")
            }
            WorkspaceConstraints::SettingsSize => {
                ErrorKind::BadRequest.with_message("Workspace settings size is invalid")
            }
        };

        error.with_resource("workspace")
    }
}

impl From<WorkspaceMemberConstraints> for Error<'static> {
    fn from(c: WorkspaceMemberConstraints) -> Self {
        let error = match c {
            WorkspaceMemberConstraints::MembershipUnique => ErrorKind::Conflict
                .with_message("This account is already a member of the workspace"),
        };

        error.with_resource("workspace_member")
    }
}

impl From<WorkspaceThreadConstraints> for Error<'static> {
    fn from(c: WorkspaceThreadConstraints) -> Self {
        let error = match c {
            WorkspaceThreadConstraints::DisplayNameLength => ErrorKind::BadRequest
                .with_message("Thread title must be between 1 and 255 characters"),
            WorkspaceThreadConstraints::ClosedConsistent => ErrorKind::InternalServerError
                .with_message("Thread open/closed state is inconsistent"),
            WorkspaceThreadConstraints::ReviewStatusFile => ErrorKind::InternalServerError
                .with_message("Thread file and review-status presence are inconsistent"),
        };

        error.with_resource("workspace_thread")
    }
}

impl From<WorkspaceThreadCommentConstraints> for Error<'static> {
    fn from(c: WorkspaceThreadCommentConstraints) -> Self {
        let error = match c {
            WorkspaceThreadCommentConstraints::BodyLength => ErrorKind::BadRequest
                .with_message("Comment body must be between 1 and 10000 characters"),
        };

        error.with_resource("workspace_thread_comment")
    }
}

impl From<WorkspaceInviteConstraints> for Error<'static> {
    fn from(c: WorkspaceInviteConstraints) -> Self {
        let error = match c {
            WorkspaceInviteConstraints::InviteeEmailFormat => {
                ErrorKind::BadRequest.with_message("Invalid invitee email format")
            }
            WorkspaceInviteConstraints::WorkspaceIdIdUnique => {
                ErrorKind::Conflict.with_message("An invite with this identifier already exists")
            }
        };

        error.with_resource("workspace_invite")
    }
}

impl From<WorkspaceActivitiesConstraints> for Error<'static> {
    fn from(c: WorkspaceActivitiesConstraints) -> Self {
        let error = match c {
            WorkspaceActivitiesConstraints::ParamsSize => {
                ErrorKind::BadRequest.with_message("Activity params size is invalid")
            }
        };

        error.with_resource("workspace_activities")
    }
}

impl From<WorkspaceWebhookConstraints> for Error<'static> {
    fn from(c: WorkspaceWebhookConstraints) -> Self {
        let error = match c {
            WorkspaceWebhookConstraints::DisplayNameLength => ErrorKind::BadRequest
                .with_message("Webhook name must be between 3 and 64 characters long"),
            WorkspaceWebhookConstraints::DescriptionLength => {
                ErrorKind::BadRequest.with_message("Webhook description is too long")
            }
            WorkspaceWebhookConstraints::UrlLength => {
                ErrorKind::BadRequest.with_message("Webhook URL is too long")
            }
            WorkspaceWebhookConstraints::UrlFormat => {
                ErrorKind::BadRequest.with_message("Webhook URL must be a valid HTTPS URL")
            }
            WorkspaceWebhookConstraints::EventsNotEmpty => {
                ErrorKind::BadRequest.with_message("Webhook must have at least one event")
            }
            WorkspaceWebhookConstraints::HeadersSize => {
                ErrorKind::BadRequest.with_message("Webhook headers size is too large")
            }
            WorkspaceWebhookConstraints::WorkspaceIdIdUnique => {
                ErrorKind::Conflict.with_message("A webhook with this identifier already exists")
            }
        };

        error.with_resource("workspace_webhook")
    }
}
