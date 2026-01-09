//! Workspace-related constraint violation error handlers.

use nvisy_postgres::types::{
    WorkspaceActivitiesConstraints, WorkspaceConstraints, WorkspaceIntegrationConstraints,
    WorkspaceIntegrationRunConstraints, WorkspaceInviteConstraints, WorkspaceMemberConstraints,
    WorkspaceWebhookConstraints,
};

use crate::handler::{Error, ErrorKind};

impl From<WorkspaceConstraints> for Error<'static> {
    fn from(c: WorkspaceConstraints) -> Self {
        let error = match c {
            WorkspaceConstraints::DisplayNameLength => ErrorKind::BadRequest
                .with_message("Workspace name must be between 3 and 32 characters long"),
            WorkspaceConstraints::DescriptionLengthMax => {
                ErrorKind::BadRequest.with_message("Workspace description is too long")
            }
            WorkspaceConstraints::KeepForSecRange => ErrorKind::BadRequest
                .with_message("Retention period must be between 1 hour and 1 year"),
            WorkspaceConstraints::MaxMembersMin => ErrorKind::InternalServerError.into_error(),
            WorkspaceConstraints::MaxMembersMax => {
                ErrorKind::BadRequest.with_message("Maximum number of members exceeded")
            }
            WorkspaceConstraints::MaxStorageMin => ErrorKind::InternalServerError.into_error(),
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
            | WorkspaceConstraints::DeletedAfterUpdated
            | WorkspaceConstraints::ArchivedAfterCreated
            | WorkspaceConstraints::DeletedAfterArchived => {
                ErrorKind::InternalServerError.into_error()
            }
            WorkspaceConstraints::ActiveStatusNotArchived => {
                ErrorKind::BadRequest.with_message("Active workspaces cannot be archived")
            }
            WorkspaceConstraints::ArchiveStatusConsistency => {
                ErrorKind::BadRequest.with_message("Workspace archive status is inconsistent")
            }
        };

        error.with_resource("workspace")
    }
}

impl From<WorkspaceMemberConstraints> for Error<'static> {
    fn from(c: WorkspaceMemberConstraints) -> Self {
        let error = match c {
            WorkspaceMemberConstraints::CustomPermissionsSize => {
                ErrorKind::BadRequest.with_message("Custom permissions data size is invalid")
            }
            WorkspaceMemberConstraints::ShowOrderRange => ErrorKind::BadRequest
                .with_message("Show order value must be between -1000 and 1000"),
            WorkspaceMemberConstraints::UpdatedAfterCreated
            | WorkspaceMemberConstraints::LastAccessedAfterCreated => {
                ErrorKind::InternalServerError.into_error()
            }
        };

        error.with_resource("workspace_member")
    }
}

impl From<WorkspaceInviteConstraints> for Error<'static> {
    fn from(c: WorkspaceInviteConstraints) -> Self {
        let error = match c {
            WorkspaceInviteConstraints::InviteMessageLengthMax => {
                ErrorKind::BadRequest.with_message("Invite message is too long")
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

impl From<WorkspaceIntegrationConstraints> for Error<'static> {
    fn from(c: WorkspaceIntegrationConstraints) -> Self {
        let error = match c {
            WorkspaceIntegrationConstraints::IntegrationNameNotEmpty => {
                ErrorKind::BadRequest.with_message("Integration name cannot be empty")
            }
            WorkspaceIntegrationConstraints::DescriptionLengthMax => {
                ErrorKind::BadRequest.with_message("Integration description is too long")
            }
            WorkspaceIntegrationConstraints::MetadataSize => {
                ErrorKind::BadRequest.with_message("Integration metadata size is invalid")
            }
            WorkspaceIntegrationConstraints::CredentialsSize => {
                ErrorKind::BadRequest.with_message("Integration credentials size is invalid")
            }
            WorkspaceIntegrationConstraints::UpdatedAfterCreated
            | WorkspaceIntegrationConstraints::LastSyncAfterCreated => {
                ErrorKind::InternalServerError.into_error()
            }
        };

        error.with_resource("workspace_integration")
    }
}

impl From<WorkspaceIntegrationRunConstraints> for Error<'static> {
    fn from(c: WorkspaceIntegrationRunConstraints) -> Self {
        let error = match c {
            WorkspaceIntegrationRunConstraints::MetadataSize => {
                ErrorKind::BadRequest.with_message("Run metadata size is invalid")
            }
            WorkspaceIntegrationRunConstraints::CompletedAfterStarted => {
                ErrorKind::InternalServerError.into_error()
            }
        };

        error.with_resource("workspace_integration_run")
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
            WorkspaceWebhookConstraints::SecretLength => {
                ErrorKind::BadRequest.with_message("Webhook secret length is invalid")
            }
            WorkspaceWebhookConstraints::EventsNotEmpty => {
                ErrorKind::BadRequest.with_message("Webhook must have at least one event")
            }
            WorkspaceWebhookConstraints::HeadersSize => {
                ErrorKind::BadRequest.with_message("Webhook headers size is too large")
            }
            WorkspaceWebhookConstraints::FailureCountPositive
            | WorkspaceWebhookConstraints::MaxFailuresPositive => {
                ErrorKind::InternalServerError.into_error()
            }
            WorkspaceWebhookConstraints::UpdatedAfterCreated
            | WorkspaceWebhookConstraints::DeletedAfterCreated => {
                ErrorKind::InternalServerError.into_error()
            }
        };

        error.with_resource("workspace_webhook")
    }
}
