//! Pipeline-related constraint violation error handlers.

use nvisy_postgres::types::{
    WorkspaceConnectionConstraints, WorkspaceConnectionSyncConstraints,
    WorkspaceDetectionConstraints, WorkspacePipelineConstraints,
    WorkspacePipelineReferenceConstraints, WorkspacePolicyConstraints,
};

use super::{Error, ErrorKind};

impl From<WorkspacePipelineConstraints> for Error<'static> {
    fn from(c: WorkspacePipelineConstraints) -> Self {
        match c {
            WorkspacePipelineConstraints::NameLength => ErrorKind::BadRequest
                .with_message("Pipeline name must be between 2 and 128 characters long"),
            WorkspacePipelineConstraints::DescriptionLength => ErrorKind::BadRequest
                .with_message("Pipeline description must be at most 500 characters long"),
            WorkspacePipelineConstraints::DefinitionSize => {
                ErrorKind::BadRequest.with_message("Pipeline definition size exceeds maximum limit")
            }
            WorkspacePipelineConstraints::MetadataSize => {
                ErrorKind::BadRequest.with_message("Pipeline metadata size exceeds maximum limit")
            }
            WorkspacePipelineConstraints::WorkspaceIdIdUnique => {
                ErrorKind::Conflict.with_message("A pipeline with this identifier already exists")
            }
        }
    }
}

impl From<WorkspaceDetectionConstraints> for Error<'static> {
    fn from(c: WorkspaceDetectionConstraints) -> Self {
        match c {
            WorkspaceDetectionConstraints::MetadataSize => {
                ErrorKind::BadRequest.with_message("Detection metadata size exceeds maximum limit")
            }
            WorkspaceDetectionConstraints::IdempotencyKeyLength => {
                ErrorKind::BadRequest.with_message("Idempotency key must be 1 to 255 characters")
            }
            WorkspaceDetectionConstraints::IdempotencyUnique => ErrorKind::Conflict
                .with_message("A detection with this idempotency key already exists"),
        }
    }
}

impl From<WorkspacePipelineReferenceConstraints> for Error<'static> {
    fn from(c: WorkspacePipelineReferenceConstraints) -> Self {
        match c {
            WorkspacePipelineReferenceConstraints::PolicyReference => ErrorKind::BadRequest
                .with_message("Referenced policy does not exist in this workspace"),
        }
    }
}

impl From<WorkspaceConnectionConstraints> for Error<'static> {
    fn from(c: WorkspaceConnectionConstraints) -> Self {
        match c {
            WorkspaceConnectionConstraints::NameLength => ErrorKind::BadRequest
                .with_message("Connection name must be between 1 and 255 characters"),
            WorkspaceConnectionConstraints::ProviderLength => ErrorKind::BadRequest
                .with_message("Provider name must be between 1 and 64 characters"),
            WorkspaceConnectionConstraints::DataSize => {
                ErrorKind::BadRequest.with_message("Connection data size exceeds maximum limit")
            }
            WorkspaceConnectionConstraints::MetadataSize => {
                ErrorKind::BadRequest.with_message("Connection metadata size exceeds maximum limit")
            }
            WorkspaceConnectionConstraints::ScheduleCronLength => {
                ErrorKind::BadRequest.with_message("Connection schedule cron length is invalid")
            }
            WorkspaceConnectionConstraints::WorkspaceIdIdUnique => {
                ErrorKind::Conflict.with_message("A connection with this identifier already exists")
            }
            WorkspaceConnectionConstraints::NameUnique => {
                ErrorKind::Conflict.with_message("A connection with this name already exists")
            }
        }
    }
}

impl From<WorkspaceConnectionSyncConstraints> for Error<'static> {
    fn from(c: WorkspaceConnectionSyncConstraints) -> Self {
        match c {
            WorkspaceConnectionSyncConstraints::ErrorMessageLength => ErrorKind::BadRequest
                .with_message("Sync error message must be between 1 and 4096 characters"),
            WorkspaceConnectionSyncConstraints::MetadataSize => {
                ErrorKind::BadRequest.with_message("Sync run metadata size exceeds maximum limit")
            }
            WorkspaceConnectionSyncConstraints::OneActivePerConnection => {
                ErrorKind::Conflict.with_message("A sync is already in progress")
            }
        }
    }
}

impl From<WorkspacePolicyConstraints> for Error<'static> {
    fn from(c: WorkspacePolicyConstraints) -> Self {
        match c {
            WorkspacePolicyConstraints::NameLength => ErrorKind::BadRequest
                .with_message("Policy name must be between 1 and 255 characters"),
            WorkspacePolicyConstraints::DescriptionLength => ErrorKind::BadRequest
                .with_message("Policy description must be at most 4096 characters"),
            WorkspacePolicyConstraints::DefinitionSize => {
                ErrorKind::BadRequest.with_message("Policy definition size exceeds maximum limit")
            }
            WorkspacePolicyConstraints::MetadataSize => {
                ErrorKind::BadRequest.with_message("Policy metadata size exceeds maximum limit")
            }
            WorkspacePolicyConstraints::NameUnique => {
                ErrorKind::Conflict.with_message("A policy with this name already exists")
            }
            WorkspacePolicyConstraints::WorkspaceIdIdUnique => {
                ErrorKind::Conflict.with_message("A policy with this identifier already exists")
            }
        }
    }
}
