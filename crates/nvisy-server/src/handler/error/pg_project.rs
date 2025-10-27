//! Project-related constraint violation error handlers.

use nvisy_postgres::types::{
    ProjectActivityLogConstraints, ProjectConstraints, ProjectInviteConstraints,
    ProjectMemberConstraints,
};

use crate::handler::{Error, ErrorKind};

impl From<ProjectConstraints> for Error<'static> {
    fn from(c: ProjectConstraints) -> Self {
        let error = match c {
            ProjectConstraints::DisplayNameLengthMin => ErrorKind::BadRequest
                .with_message("Project name must be at least 3 characters long"),
            ProjectConstraints::DisplayNameLengthMax => {
                ErrorKind::BadRequest.with_message("Project name cannot exceed 32 characters")
            }
            ProjectConstraints::DescriptionLengthMax => {
                ErrorKind::BadRequest.with_message("Project description is too long")
            }
            ProjectConstraints::ProjectCodeFormat => {
                ErrorKind::BadRequest.with_message("Invalid project code format")
            }
            ProjectConstraints::CategoryLengthMax => {
                ErrorKind::BadRequest.with_message("Project category is too long")
            }
            ProjectConstraints::TagsCountMax => ErrorKind::BadRequest.with_message("Too many tags"),
            ProjectConstraints::KeepForSecMin => ErrorKind::BadRequest
                .with_message("Retention period must be greater than 60 seconds"),
            ProjectConstraints::KeepForSecMax => {
                ErrorKind::BadRequest.with_message("Retention period cannot exceed 7 days")
            }
            ProjectConstraints::MaxMembersMin => ErrorKind::InternalServerError.into_error(),
            ProjectConstraints::MaxMembersMax => {
                ErrorKind::BadRequest.with_message("Maximum number of members exceeded")
            }
            ProjectConstraints::MaxDocumentsMin => ErrorKind::InternalServerError.into_error(),
            ProjectConstraints::MaxStorageMbMin => ErrorKind::InternalServerError.into_error(),
            ProjectConstraints::MetadataSizeMin => ErrorKind::InternalServerError.into_error(),
            ProjectConstraints::MetadataSizeMax => {
                ErrorKind::BadRequest.with_message("Project metadata exceeds maximum allowed size")
            }
            ProjectConstraints::SettingsSizeMin => ErrorKind::InternalServerError.into_error(),
            ProjectConstraints::SettingsSizeMax => {
                ErrorKind::BadRequest.with_message("Project settings exceed maximum allowed size")
            }
            ProjectConstraints::UpdatedAfterCreated
            | ProjectConstraints::DeletedAfterCreated
            | ProjectConstraints::DeletedAfterUpdated
            | ProjectConstraints::ArchivedAfterCreated
            | ProjectConstraints::DeletedAfterArchived => {
                ErrorKind::InternalServerError.into_error()
            }
            ProjectConstraints::TemplateCannotHaveTemplate => {
                ErrorKind::BadRequest.with_message("Templates cannot be based on other templates")
            }
            ProjectConstraints::ActiveStatusNotArchived => {
                ErrorKind::BadRequest.with_message("Active projects cannot be archived")
            }
            ProjectConstraints::ArchiveStatusConsistency => {
                ErrorKind::BadRequest.with_message("Project archive status is inconsistent")
            }
            ProjectConstraints::DisplayNameOwnerUnique => {
                ErrorKind::Conflict.with_message("A project with this name already exists")
            }
            ProjectConstraints::ProjectCodeUnique => {
                ErrorKind::Conflict.with_message("A project with this code already exists")
            }
        };

        error.with_resource("project")
    }
}

impl From<ProjectMemberConstraints> for Error<'static> {
    fn from(c: ProjectMemberConstraints) -> Self {
        let error = match c {
            ProjectMemberConstraints::CustomPermissionsSizeMin => {
                ErrorKind::InternalServerError.into_error()
            }
            ProjectMemberConstraints::CustomPermissionsSizeMax => {
                ErrorKind::BadRequest.with_message("Custom permissions data is too large")
            }
            ProjectMemberConstraints::ShowOrderMin => {
                ErrorKind::BadRequest.with_message("Invalid show order value")
            }
            ProjectMemberConstraints::ShowOrderMax => {
                ErrorKind::BadRequest.with_message("Show order value is too large")
            }
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
            ProjectInviteConstraints::EmailValid => {
                ErrorKind::BadRequest.with_message("Invalid email address")
            }
            ProjectInviteConstraints::InviteMessageLengthMax => {
                ErrorKind::BadRequest.with_message("Invite message is too long")
            }
            ProjectInviteConstraints::InviteTokenNotEmpty => {
                ErrorKind::InternalServerError.into_error()
            }
            ProjectInviteConstraints::StatusReasonLengthMax => {
                ErrorKind::BadRequest.with_message("Status reason is too long")
            }
            ProjectInviteConstraints::MaxUsesMin => {
                ErrorKind::BadRequest.with_message("Maximum uses must be at least 1")
            }
            ProjectInviteConstraints::MaxUsesMax => {
                ErrorKind::BadRequest.with_message("Maximum uses exceeded limit")
            }
            ProjectInviteConstraints::UseCountMin => ErrorKind::InternalServerError.into_error(),
            ProjectInviteConstraints::UseCountMax => {
                ErrorKind::BadRequest.with_message("Invite has been used too many times")
            }
            ProjectInviteConstraints::ExpiresInFuture => {
                ErrorKind::BadRequest.with_message("Invite expiration must be in the future")
            }
            ProjectInviteConstraints::ExpiresAfterCreated
            | ProjectInviteConstraints::UpdatedAfterCreated
            | ProjectInviteConstraints::DeletedAfterUpdated
            | ProjectInviteConstraints::AcceptedAfterCreated => {
                ErrorKind::InternalServerError.into_error()
            }
            ProjectInviteConstraints::AcceptStatusConsistency => {
                ErrorKind::BadRequest.with_message("Invite acceptance status is inconsistent")
            }
            ProjectInviteConstraints::AcceptorConsistency => {
                ErrorKind::BadRequest.with_message("Invite acceptor information is inconsistent")
            }
            ProjectInviteConstraints::TokenUnique => {
                ErrorKind::Conflict.with_message("Invite token already exists")
            }
        };

        error.with_resource("project_invite")
    }
}

impl From<ProjectActivityLogConstraints> for Error<'static> {
    fn from(c: ProjectActivityLogConstraints) -> Self {
        let error = match c {
            ProjectActivityLogConstraints::ActivityTypeNotEmpty => {
                ErrorKind::BadRequest.with_message("Activity type cannot be empty")
            }
            ProjectActivityLogConstraints::ActivityTypeLengthMax => {
                ErrorKind::BadRequest.with_message("Activity type is too long")
            }
            ProjectActivityLogConstraints::ActivityDataSizeMin => {
                ErrorKind::InternalServerError.into_error()
            }
            ProjectActivityLogConstraints::ActivityDataSizeMax => {
                ErrorKind::BadRequest.with_message("Activity data is too large")
            }
            ProjectActivityLogConstraints::EntityTypeLengthMax => {
                ErrorKind::BadRequest.with_message("Entity type is too long")
            }
        };

        error.with_resource("project_activity_log")
    }
}
