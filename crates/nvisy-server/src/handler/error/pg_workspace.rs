//! Workspace-related constraint violation error handlers.

use nvisy_postgres::types::{
    WorkspaceActivitiesConstraints, WorkspaceConstraints, WorkspaceInviteConstraints,
    WorkspaceMemberConstraints, WorkspaceWebhookConstraints,
};

use crate::handler::{Error, ErrorKind};

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
            WorkspaceConstraints::SlugUnique => {
                ErrorKind::Conflict.with_message("A workspace with this slug already exists")
            }
            WorkspaceConstraints::DescriptionLengthMax => {
                ErrorKind::BadRequest.with_message("Workspace description is too long")
            }
            WorkspaceConstraints::TagsCountMax => {
                ErrorKind::BadRequest.with_message("Too many tags")
            }
            WorkspaceConstraints::MetadataSize => {
                ErrorKind::BadRequest.with_message("Workspace metadata size is invalid")
            }
            WorkspaceConstraints::SettingsSize => {
                ErrorKind::BadRequest.with_message("Workspace settings size is invalid")
            }
            WorkspaceConstraints::UpdatedAfterCreated
            | WorkspaceConstraints::DeletedAfterCreated
            | WorkspaceConstraints::DeletedAfterUpdated => {
                ErrorKind::InternalServerError.into_error()
            }
        };

        error.with_resource("workspace")
    }
}

impl From<WorkspaceMemberConstraints> for Error<'static> {
    fn from(c: WorkspaceMemberConstraints) -> Self {
        let error = match c {
            WorkspaceMemberConstraints::UpdatedAfterCreated => {
                ErrorKind::InternalServerError.into_error()
            }
        };

        error.with_resource("workspace_member")
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
            WorkspaceInviteConstraints::InviteTokenNotEmpty => {
                ErrorKind::InternalServerError.into_error()
            }
            WorkspaceInviteConstraints::ExpiresAfterCreated
            | WorkspaceInviteConstraints::UpdatedAfterCreated
            | WorkspaceInviteConstraints::RespondedAfterCreated => {
                ErrorKind::InternalServerError.into_error()
            }
        };

        error.with_resource("workspace_invite")
    }
}

impl From<WorkspaceActivitiesConstraints> for Error<'static> {
    fn from(c: WorkspaceActivitiesConstraints) -> Self {
        let error = match c {
            WorkspaceActivitiesConstraints::DescriptionLengthMax => {
                ErrorKind::BadRequest.with_message("Activity description is too long")
            }
            WorkspaceActivitiesConstraints::MetadataSize => {
                ErrorKind::BadRequest.with_message("Activity metadata size is invalid")
            }
        };

        error.with_resource("workspace_activities")
    }
}

impl From<WorkspaceWebhookConstraints> for Error<'static> {
    fn from(c: WorkspaceWebhookConstraints) -> Self {
        let error = match c {
            WorkspaceWebhookConstraints::SlugLength => ErrorKind::BadRequest
                .with_message("Webhook slug must be between 3 and 32 characters long"),
            WorkspaceWebhookConstraints::SlugFormat => ErrorKind::BadRequest.with_message(
                "Webhook slug must be lowercase alphanumeric with single internal dashes",
            ),
            WorkspaceWebhookConstraints::SlugUnique => {
                ErrorKind::Conflict.with_message("A webhook with this slug already exists")
            }
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
            WorkspaceWebhookConstraints::UpdatedAfterCreated
            | WorkspaceWebhookConstraints::DeletedAfterCreated => {
                ErrorKind::InternalServerError.into_error()
            }
        };

        error.with_resource("workspace_webhook")
    }
}
