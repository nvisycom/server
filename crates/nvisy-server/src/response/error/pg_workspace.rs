//! Workspace-related constraint violation error handlers.

use nvisy_postgres::types::{
    WorkspaceActivitiesConstraints, WorkspaceConstraints, WorkspaceInviteConstraints,
    WorkspaceMemberConstraints, WorkspaceReviewCommentConstraints, WorkspaceReviewConstraints,
    WorkspaceWebhookConstraints,
};

use super::{Error, ErrorKind};

impl From<WorkspaceConstraints> for Error<'static> {
    fn from(c: WorkspaceConstraints) -> Self {
        match c {
            WorkspaceConstraints::DisplayNameLength => ErrorKind::BadRequest
                .with_message("Workspace name must be between 3 and 32 characters long"),
            WorkspaceConstraints::HandleLength => ErrorKind::BadRequest
                .with_message("Workspace handle must be between 3 and 32 characters long"),
            WorkspaceConstraints::HandleFormat => ErrorKind::BadRequest.with_message(
                "Workspace handle must be lowercase alphanumeric with single internal dashes",
            ),
            WorkspaceConstraints::NameUnique => {
                ErrorKind::Conflict.with_message("A workspace with this name already exists")
            }
            WorkspaceConstraints::HandleUnique => {
                ErrorKind::Conflict.with_message("A workspace with this handle already exists")
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
        }
    }
}

impl From<WorkspaceMemberConstraints> for Error<'static> {
    fn from(c: WorkspaceMemberConstraints) -> Self {
        match c {
            WorkspaceMemberConstraints::MembershipUnique => ErrorKind::Conflict
                .with_message("This account is already a member of the workspace"),
        }
    }
}

impl From<WorkspaceReviewConstraints> for Error<'static> {
    fn from(c: WorkspaceReviewConstraints) -> Self {
        match c {
            WorkspaceReviewConstraints::DisplayNameLength => ErrorKind::BadRequest
                .with_message("Review title must be between 1 and 255 characters"),
        }
    }
}

impl From<WorkspaceReviewCommentConstraints> for Error<'static> {
    fn from(c: WorkspaceReviewCommentConstraints) -> Self {
        match c {
            WorkspaceReviewCommentConstraints::BodyLength => ErrorKind::BadRequest
                .with_message("Comment body must be between 1 and 10000 characters"),
        }
    }
}

impl From<WorkspaceInviteConstraints> for Error<'static> {
    fn from(c: WorkspaceInviteConstraints) -> Self {
        match c {
            WorkspaceInviteConstraints::InviteeEmailFormat => {
                ErrorKind::BadRequest.with_message("Invalid invitee email format")
            }
            WorkspaceInviteConstraints::WorkspaceIdIdUnique => {
                ErrorKind::Conflict.with_message("An invite with this identifier already exists")
            }
        }
    }
}

impl From<WorkspaceActivitiesConstraints> for Error<'static> {
    fn from(c: WorkspaceActivitiesConstraints) -> Self {
        match c {
            WorkspaceActivitiesConstraints::ParamsSize => {
                ErrorKind::BadRequest.with_message("Activity params size is invalid")
            }
        }
    }
}

impl From<WorkspaceWebhookConstraints> for Error<'static> {
    fn from(c: WorkspaceWebhookConstraints) -> Self {
        match c {
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
        }
    }
}
