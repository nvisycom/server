//! Pipeline-related constraint violation error handlers.

use nvisy_postgres::types::{
    PipelineArtifactConstraints, PipelineConstraints, PipelineReferenceConstraints,
    PipelineRunConstraints, WorkspaceConnectionConstraints, WorkspaceConnectionRunConstraints,
    WorkspaceContextConstraints,
};

use crate::handler::{Error, ErrorKind};

impl From<PipelineConstraints> for Error<'static> {
    fn from(c: PipelineConstraints) -> Self {
        let error = match c {
            PipelineConstraints::NameLength => ErrorKind::BadRequest
                .with_message("Pipeline name must be between 1 and 255 characters long"),
            PipelineConstraints::DescriptionLength => ErrorKind::BadRequest
                .with_message("Pipeline description must be at most 4096 characters long"),
            PipelineConstraints::DefinitionSize => {
                ErrorKind::BadRequest.with_message("Pipeline definition size exceeds maximum limit")
            }
            PipelineConstraints::MetadataSize => {
                ErrorKind::BadRequest.with_message("Pipeline metadata size exceeds maximum limit")
            }
            PipelineConstraints::UpdatedAfterCreated | PipelineConstraints::DeletedAfterCreated => {
                ErrorKind::InternalServerError.into_error()
            }
        };

        error.with_resource("pipeline")
    }
}

impl From<PipelineRunConstraints> for Error<'static> {
    fn from(c: PipelineRunConstraints) -> Self {
        let error = match c {
            PipelineRunConstraints::AnalyzedDocumentKeyLength => {
                ErrorKind::InternalServerError.into_error()
            }
            PipelineRunConstraints::MetadataSize => ErrorKind::BadRequest
                .with_message("Pipeline run metadata size exceeds maximum limit"),
            PipelineRunConstraints::IdempotencyKeyLength => {
                ErrorKind::BadRequest.with_message("Idempotency key must be 1 to 255 characters")
            }
            PipelineRunConstraints::CompletedAfterStarted => {
                ErrorKind::InternalServerError.into_error()
            }
        };

        error.with_resource("pipeline_run")
    }
}

impl From<PipelineArtifactConstraints> for Error<'static> {
    fn from(c: PipelineArtifactConstraints) -> Self {
        let error = match c {
            PipelineArtifactConstraints::MetadataSize => {
                ErrorKind::BadRequest.with_message("Artifact metadata size exceeds maximum limit")
            }
        };

        error.with_resource("pipeline_artifact")
    }
}

impl From<PipelineReferenceConstraints> for Error<'static> {
    fn from(c: PipelineReferenceConstraints) -> Self {
        let (resource, error) = match c {
            PipelineReferenceConstraints::PolicyReference => (
                "policy",
                ErrorKind::BadRequest
                    .with_message("Referenced policy does not exist in this workspace"),
            ),
            PipelineReferenceConstraints::ContextReference => (
                "context",
                ErrorKind::BadRequest
                    .with_message("Referenced context does not exist in this workspace"),
            ),
            // The pipeline side of the FK only fails if the pipeline row vanished
            // mid-transaction, which is a server-side fault rather than bad input.
            PipelineReferenceConstraints::PolicyPipelineReference
            | PipelineReferenceConstraints::ContextPipelineReference => {
                ("pipeline", ErrorKind::InternalServerError.into_error())
            }
        };

        error.with_resource(resource)
    }
}

impl From<WorkspaceConnectionConstraints> for Error<'static> {
    fn from(c: WorkspaceConnectionConstraints) -> Self {
        let error = match c {
            WorkspaceConnectionConstraints::NameLength => ErrorKind::BadRequest
                .with_message("Connection name must be between 1 and 255 characters"),
            WorkspaceConnectionConstraints::ProviderLength => ErrorKind::BadRequest
                .with_message("Provider name must be between 1 and 64 characters"),
            WorkspaceConnectionConstraints::DataSize => {
                ErrorKind::BadRequest.with_message("Connection data size exceeds maximum limit")
            }
            WorkspaceConnectionConstraints::NameUnique => ErrorKind::Conflict
                .with_message("A connection with this name already exists in the workspace"),
            WorkspaceConnectionConstraints::UpdatedAfterCreated
            | WorkspaceConnectionConstraints::DeletedAfterCreated => {
                ErrorKind::InternalServerError.into_error()
            }
        };

        error.with_resource("workspace_connection")
    }
}

impl From<WorkspaceConnectionRunConstraints> for Error<'static> {
    fn from(c: WorkspaceConnectionRunConstraints) -> Self {
        let error = match c {
            WorkspaceConnectionRunConstraints::ErrorMessageLength => ErrorKind::BadRequest
                .with_message("Sync error message must be between 1 and 4096 characters"),
            WorkspaceConnectionRunConstraints::MetadataSize => {
                ErrorKind::BadRequest.with_message("Sync run metadata size exceeds maximum limit")
            }
            WorkspaceConnectionRunConstraints::RecordsSyncedNonNegative
            | WorkspaceConnectionRunConstraints::CompletedAfterStarted => {
                ErrorKind::InternalServerError.into_error()
            }
        };

        error.with_resource("workspace_connection_run")
    }
}

impl From<WorkspaceContextConstraints> for Error<'static> {
    fn from(c: WorkspaceContextConstraints) -> Self {
        let error =
            match c {
                WorkspaceContextConstraints::NameLength => ErrorKind::BadRequest
                    .with_message("Context name must be between 1 and 255 characters"),
                WorkspaceContextConstraints::DescriptionLength => ErrorKind::BadRequest
                    .with_message("Context description must be at most 4096 characters"),
                WorkspaceContextConstraints::MimeTypeLength => ErrorKind::BadRequest
                    .with_message("MIME type must be between 1 and 128 characters"),
                WorkspaceContextConstraints::StorageKeyLength => {
                    ErrorKind::BadRequest.with_message("Storage key length exceeds maximum limit")
                }
                WorkspaceContextConstraints::ContentSizePositive => {
                    ErrorKind::BadRequest.with_message("Content size must be greater than zero")
                }
                WorkspaceContextConstraints::ContentHashLength => {
                    ErrorKind::BadRequest.with_message("Content hash must be exactly 32 bytes")
                }
                WorkspaceContextConstraints::MetadataSize => ErrorKind::BadRequest
                    .with_message("Context metadata size exceeds maximum limit"),
                WorkspaceContextConstraints::NameUnique => ErrorKind::Conflict
                    .with_message("A context with this name already exists in the workspace"),
                WorkspaceContextConstraints::UpdatedAfterCreated
                | WorkspaceContextConstraints::DeletedAfterCreated => {
                    ErrorKind::InternalServerError.into_error()
                }
            };

        error.with_resource("workspace_context")
    }
}
