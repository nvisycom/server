//! Project-related constraint violation error handlers.

use nvisy_postgres::types::{
    ProjectActivitiesConstraints, ProjectConstraints, ProjectIntegrationConstraints,
    ProjectInviteConstraints, ProjectMemberConstraints,
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
