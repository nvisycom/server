//! Constraint violation to HTTP error mapping.
//!
//! One table: each violated constraint maps to an [`ErrorKind`] and, for the
//! ones a client can cause, the message it is told. A constraint with no message
//! guards an invariant the handlers already enforce, so its violation is a
//! server-side bug.

use nvisy_postgres::types::{
    AccountApiTokenConstraints, AccountConstraints, AccountIdentityConstraints,
    AccountNotificationConstraints, WorkspaceActivitiesConstraints, WorkspaceBlobConstraints,
    WorkspaceConnectionConstraints, WorkspaceConnectionSyncConstraints, WorkspaceConstraints,
    WorkspaceDetectionConstraints, WorkspaceDocumentConstraints, WorkspaceInviteConstraints,
    WorkspaceMemberConstraints, WorkspacePolicyConstraints, WorkspaceProviderConstraints,
    WorkspaceReviewCommentConstraints, WorkspaceReviewConstraints, WorkspaceWebhookConstraints,
};

use super::{Error, ErrorKind};

/// Implements `From<Constraints> for Error` for each table of `Variant =>
/// Kind("message")` (or a bare `Kind`) arms.
macro_rules! constraint_errors {
    ($($ty:ident { $($($variant:ident)|+ => $kind:ident $(($message:literal))?),+ $(,)? })+) => {$(
        impl From<$ty> for Error {
            fn from(constraint: $ty) -> Self {
                match constraint {
                    $($($ty::$variant)|+ => ErrorKind::$kind.into_error()$(.with_message($message))?,)+
                }
            }
        }
    )+};
}

constraint_errors! {
    AccountConstraints {
        UsernameLength => BadRequest("Handle must be between 3 and 32 characters long"),
        UsernameFormat => BadRequest("Handle must be lowercase alphanumeric with single internal dashes"),
        EmailUnique => Conflict("An account with this email already exists"),
        UsernameUnique => Conflict("Handle is already taken"),
        DisplayNameLength => BadRequest("Display name must be between 2 and 32 characters long"),
        DisplayNameNotEmpty => BadRequest("Display name cannot be empty"),
        EmailFormat => BadRequest("Invalid email format"),
        EmailLengthMax => BadRequest("Email address is too long"),
        TimezoneFormat => BadRequest("Invalid timezone format"),
        LocaleFormat => BadRequest("Invalid locale format"),
    }
    AccountIdentityConstraints {
        PasswordShape | OidcShape | AccountProviderUnique => InternalServerError,
        ProviderSubjectUnique => Conflict("This identity is already linked to another account"),
    }
    AccountApiTokenConstraints {
        NameNotEmpty => BadRequest("Token name cannot be empty"),
        NameLength => BadRequest("Token name is too long"),
    }
    AccountNotificationConstraints {
        ParamsSize => BadRequest("Notification params must be between 2 and 4096 bytes"),
    }
    WorkspaceConstraints {
        DisplayNameLength => BadRequest("Workspace name must be between 3 and 32 characters long"),
        HandleLength => BadRequest("Workspace handle must be between 3 and 32 characters long"),
        HandleFormat => BadRequest("Workspace handle must be lowercase alphanumeric with single internal dashes"),
        NameUnique => Conflict("A workspace with this name already exists"),
        HandleUnique => Conflict("A workspace with this handle already exists"),
        DescriptionLengthMax => BadRequest("Workspace description is too long"),
        MetadataSize => BadRequest("Workspace metadata size is invalid"),
        SettingsSize => BadRequest("Workspace settings size is invalid"),
    }
    WorkspaceMemberConstraints {
        MembershipUnique => Conflict("This account is already a member of the workspace"),
    }
    WorkspaceReviewConstraints {
        DisplayNameLength => BadRequest("Review title must be between 1 and 255 characters"),
    }
    WorkspaceReviewCommentConstraints {
        BodyLength => BadRequest("Comment body must be between 1 and 10000 characters"),
    }
    WorkspaceInviteConstraints {
        InviteeEmailFormat => BadRequest("Invalid invitee email format"),
    }
    WorkspaceActivitiesConstraints {
        ParamsSize => BadRequest("Activity params size is invalid"),
    }
    WorkspaceWebhookConstraints {
        DisplayNameLength => BadRequest("Webhook name must be between 3 and 64 characters long"),
        DescriptionLength => BadRequest("Webhook description is too long"),
        UrlLength => BadRequest("Webhook URL is too long"),
        UrlFormat => BadRequest("Webhook URL must be a valid HTTPS URL"),
        EventsNotEmpty => BadRequest("Webhook must have at least one event"),
        HeadersSize => BadRequest("Webhook headers size is too large"),
    }
    WorkspaceDocumentConstraints {
        DisplayNameLength => BadRequest("Document name must be between 1 and 255 characters long"),
        OriginalFilenameLength => BadRequest("Original filename must be between 1 and 255 characters long"),
        FileExtensionFormat => BadRequest("Invalid file extension format"),
        MetadataSize => BadRequest("Document metadata size is invalid"),
        WorkspaceIdIdUnique => Conflict("A document with this identifier already exists"),
    }
    WorkspaceBlobConstraints {
        FileSizeMin => BadRequest("File size must be greater than or equal to 0"),
        ContentHashLength | StoragePathNotEmpty | StorageBucketNotEmpty | RefCountMin | AuditDigestLength | PurgedAfterCreated | ReclaimedAfterPurged => InternalServerError("Invalid blob state"),
    }
    WorkspaceDetectionConstraints {
        MetadataSize => BadRequest("Detection metadata size exceeds maximum limit"),
        IdempotencyKeyLength => BadRequest("Idempotency key must be 1 to 255 characters"),
        IdempotencyUnique => Conflict("A detection with this idempotency key already exists"),
    }
    WorkspaceConnectionConstraints {
        NameLength => BadRequest("Connection name must be between 1 and 255 characters"),
        ProviderLength => BadRequest("Provider name must be between 1 and 64 characters"),
        DataSize => BadRequest("Connection data size exceeds maximum limit"),
        MetadataSize => BadRequest("Connection metadata size exceeds maximum limit"),
        ScheduleCronLength => BadRequest("Connection schedule cron length is invalid"),
        NameUnique => Conflict("A connection with this name already exists"),
    }
    WorkspaceConnectionSyncConstraints {
        ErrorMessageLength => BadRequest("Sync error message must be between 1 and 4096 characters"),
        MetadataSize => BadRequest("Sync run metadata size exceeds maximum limit"),
        OneActivePerConnection => Conflict("A sync is already in progress"),
    }
    WorkspacePolicyConstraints {
        NameLength => BadRequest("Policy name must be between 1 and 255 characters"),
        DescriptionLength => BadRequest("Policy description must be at most 4096 characters"),
        DefinitionSize => BadRequest("Policy definition size exceeds maximum limit"),
        MetadataSize => BadRequest("Policy metadata size exceeds maximum limit"),
        VersionUnique => Conflict("The policy was changed concurrently; reload it and try again"),
        NameUnique => Conflict("A policy with this name already exists"),
        WorkspaceIdIdUnique => Conflict("A policy with this identifier already exists"),
    }
    WorkspaceProviderConstraints {
        NameLength => BadRequest("Provider name must be between 1 and 255 characters"),
        ProviderLength => BadRequest("Provider kind must be between 1 and 64 characters"),
        DataSize => BadRequest("Provider data size exceeds maximum limit"),
        MetadataSize => BadRequest("Provider metadata size exceeds maximum limit"),
        NameUnique => Conflict("A provider with this name already exists"),
    }
}
