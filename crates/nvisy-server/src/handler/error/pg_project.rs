//! Project-related constraint violation error handlers.

use nvisy_postgres::types::{
    ProjectActivitiesConstraints, ProjectConstraints, ProjectIntegrationConstraints,
    ProjectInviteConstraints, ProjectMemberConstraints, ProjectRunConstraints,
    ProjectWebhookConstraints,
};

use crate::handler::{Error, ErrorKind};

impl From<ProjectConstraints> for Error<'static> {
    fn from(c: ProjectConstraints) -> Self {
        let error = match c {
            ProjectConstraints::DisplayNameLength => ErrorKind::BadRequest
                .with_message("Project name must be between 3 and 32 characters long"),
            ProjectConstraints::DescriptionLengthMax => {
                ErrorKind::BadRequest.with_message("Project description is too long")
            }
            ProjectConstraints::KeepForSecRange => ErrorKind::BadRequest
                .with_message("Retention period must be between 1 hour and 1 year"),
            ProjectConstraints::MaxMembersMin => ErrorKind::InternalServerError.into_error(),
            ProjectConstraints::MaxMembersMax => {
                ErrorKind::BadRequest.with_message("Maximum number of members exceeded")
            }
            ProjectConstraints::MaxStorageMin => ErrorKind::InternalServerError.into_error(),
            ProjectConstraints::TagsCountMax => ErrorKind::BadRequest.with_message("Too many tags"),
            ProjectConstraints::MetadataSize => {
                ErrorKind::BadRequest.with_message("Project metadata size is invalid")
            }
            ProjectConstraints::SettingsSize => {
                ErrorKind::BadRequest.with_message("Project settings size is invalid")
            }
            ProjectConstraints::UpdatedAfterCreated
            | ProjectConstraints::DeletedAfterCreated
            | ProjectConstraints::DeletedAfterUpdated
            | ProjectConstraints::ArchivedAfterCreated
            | ProjectConstraints::DeletedAfterArchived => {
                ErrorKind::InternalServerError.into_error()
            }
            ProjectConstraints::ActiveStatusNotArchived => {
                ErrorKind::BadRequest.with_message("Active projects cannot be archived")
            }
            ProjectConstraints::ArchiveStatusConsistency => {
                ErrorKind::BadRequest.with_message("Project archive status is inconsistent")
            }
        };

        error.with_resource("project")
    }
}

impl From<ProjectMemberConstraints> for Error<'static> {
    fn from(c: ProjectMemberConstraints) -> Self {
        let error = match c {
            ProjectMemberConstraints::CustomPermissionsSize => {
                ErrorKind::BadRequest.with_message("Custom permissions data size is invalid")
            }
            ProjectMemberConstraints::ShowOrderRange => ErrorKind::BadRequest
                .with_message("Show order value must be between -1000 and 1000"),
            ProjectMemberConstraints::UpdatedAfterCreated
            | ProjectMemberConstraints::LastAccessedAfterCreated => {
                ErrorKind::InternalServerError.into_error()
            }
        };

        error.with_resource("project_member")
    }
}

impl From<ProjectInviteConstraints> for Error<'static> {
    fn from(c: ProjectInviteConstraints) -> Self {
        let error = match c {
            ProjectInviteConstraints::InviteMessageLengthMax => {
                ErrorKind::BadRequest.with_message("Invite message is too long")
            }
            ProjectInviteConstraints::InviteTokenNotEmpty => {
                ErrorKind::InternalServerError.into_error()
            }
            ProjectInviteConstraints::ExpiresAfterCreated
            | ProjectInviteConstraints::UpdatedAfterCreated
            | ProjectInviteConstraints::RespondedAfterCreated => {
                ErrorKind::InternalServerError.into_error()
            }
        };

        error.with_resource("project_invite")
    }
}

impl From<ProjectActivitiesConstraints> for Error<'static> {
    fn from(c: ProjectActivitiesConstraints) -> Self {
        let error = match c {
            ProjectActivitiesConstraints::DescriptionLengthMax => {
                ErrorKind::BadRequest.with_message("Activity description is too long")
            }
            ProjectActivitiesConstraints::MetadataSize => {
                ErrorKind::BadRequest.with_message("Activity metadata size is invalid")
            }
        };

        error.with_resource("project_activities")
    }
}

impl From<ProjectIntegrationConstraints> for Error<'static> {
    fn from(c: ProjectIntegrationConstraints) -> Self {
        let error = match c {
            ProjectIntegrationConstraints::IntegrationNameNotEmpty => {
                ErrorKind::BadRequest.with_message("Integration name cannot be empty")
            }
            ProjectIntegrationConstraints::DescriptionLengthMax => {
                ErrorKind::BadRequest.with_message("Integration description is too long")
            }
            ProjectIntegrationConstraints::MetadataSize => {
                ErrorKind::BadRequest.with_message("Integration metadata size is invalid")
            }
            ProjectIntegrationConstraints::CredentialsSize => {
                ErrorKind::BadRequest.with_message("Integration credentials size is invalid")
            }
            ProjectIntegrationConstraints::UpdatedAfterCreated
            | ProjectIntegrationConstraints::LastSyncAfterCreated => {
                ErrorKind::InternalServerError.into_error()
            }
        };

        error.with_resource("project_integration")
    }
}

impl From<ProjectRunConstraints> for Error<'static> {
    fn from(c: ProjectRunConstraints) -> Self {
        let error = match c {
            ProjectRunConstraints::RunNameLength => {
                ErrorKind::BadRequest.with_message("Run name length is invalid")
            }
            ProjectRunConstraints::RunTypeFormat => {
                ErrorKind::BadRequest.with_message("Run type format is invalid")
            }
            ProjectRunConstraints::DurationPositive => {
                ErrorKind::BadRequest.with_message("Run duration must be positive")
            }
            ProjectRunConstraints::ResultSummaryLength => {
                ErrorKind::BadRequest.with_message("Run result summary is too long")
            }
            ProjectRunConstraints::MetadataSize => {
                ErrorKind::BadRequest.with_message("Run metadata size is invalid")
            }
            ProjectRunConstraints::ErrorDetailsSize => {
                ErrorKind::BadRequest.with_message("Run error details size is invalid")
            }
            ProjectRunConstraints::CompletedAfterStarted
            | ProjectRunConstraints::UpdatedAfterCreated
            | ProjectRunConstraints::StartedAfterCreated => {
                ErrorKind::InternalServerError.into_error()
            }
        };

        error.with_resource("project_run")
    }
}

impl From<ProjectWebhookConstraints> for Error<'static> {
    fn from(c: ProjectWebhookConstraints) -> Self {
        let error = match c {
            ProjectWebhookConstraints::DisplayNameLength => ErrorKind::BadRequest
                .with_message("Webhook name must be between 3 and 64 characters long"),
            ProjectWebhookConstraints::DescriptionLength => {
                ErrorKind::BadRequest.with_message("Webhook description is too long")
            }
            ProjectWebhookConstraints::UrlLength => {
                ErrorKind::BadRequest.with_message("Webhook URL is too long")
            }
            ProjectWebhookConstraints::UrlFormat => {
                ErrorKind::BadRequest.with_message("Webhook URL must be a valid HTTPS URL")
            }
            ProjectWebhookConstraints::SecretLength => {
                ErrorKind::BadRequest.with_message("Webhook secret length is invalid")
            }
            ProjectWebhookConstraints::EventsNotEmpty => {
                ErrorKind::BadRequest.with_message("Webhook must have at least one event")
            }
            ProjectWebhookConstraints::HeadersSize => {
                ErrorKind::BadRequest.with_message("Webhook headers size is too large")
            }
            ProjectWebhookConstraints::FailureCountPositive
            | ProjectWebhookConstraints::MaxFailuresPositive => {
                ErrorKind::InternalServerError.into_error()
            }
            ProjectWebhookConstraints::UpdatedAfterCreated
            | ProjectWebhookConstraints::DeletedAfterCreated => {
                ErrorKind::InternalServerError.into_error()
            }
        };

        error.with_resource("project_webhook")
    }
}
