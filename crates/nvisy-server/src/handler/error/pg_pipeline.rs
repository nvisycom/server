//! Pipeline-related constraint violation error handlers.

use nvisy_postgres::types::{
    PipelineArtifactConstraints, PipelineConstraints, PipelineRunConstraints,
    WorkspaceConnectionConstraints,
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
            PipelineRunConstraints::InputConfigSize => ErrorKind::BadRequest
                .with_message("Pipeline run input configuration size exceeds maximum limit"),
            PipelineRunConstraints::OutputConfigSize => ErrorKind::BadRequest
                .with_message("Pipeline run output configuration size exceeds maximum limit"),
            PipelineRunConstraints::DefinitionSnapshotSize => ErrorKind::BadRequest
                .with_message("Pipeline run definition snapshot size exceeds maximum limit"),
            PipelineRunConstraints::ErrorSize => ErrorKind::BadRequest
                .with_message("Pipeline run error details size exceeds maximum limit"),
            PipelineRunConstraints::MetricsSize => ErrorKind::BadRequest
                .with_message("Pipeline run metrics size exceeds maximum limit"),
            PipelineRunConstraints::StartedAfterCreated
            | PipelineRunConstraints::CompletedAfterStarted => {
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
