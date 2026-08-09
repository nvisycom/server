//! Pipeline-related constraint violation error handlers.

use nvisy_postgres::types::{
    WorkspaceConnectionConstraints, WorkspaceConnectionSyncConstraints,
    WorkspacePipelineArtifactConstraints, WorkspacePipelineConstraints,
    WorkspacePipelineReferenceConstraints, WorkspacePipelineRunConstraints,
    WorkspacePolicyConstraints,
};

use crate::handler::{Error, ErrorKind};

impl From<WorkspacePipelineConstraints> for Error<'static> {
    fn from(c: WorkspacePipelineConstraints) -> Self {
        let error =
            match c {
                WorkspacePipelineConstraints::NameLength => ErrorKind::BadRequest
                    .with_message("Pipeline name must be between 2 and 128 characters long"),
                WorkspacePipelineConstraints::DescriptionLength => ErrorKind::BadRequest
                    .with_message("Pipeline description must be at most 500 characters long"),
                WorkspacePipelineConstraints::DefinitionSize => ErrorKind::BadRequest
                    .with_message("Pipeline definition size exceeds maximum limit"),
                WorkspacePipelineConstraints::MetadataSize => ErrorKind::BadRequest
                    .with_message("Pipeline metadata size exceeds maximum limit"),
                WorkspacePipelineConstraints::ScheduleCronLength => {
                    ErrorKind::BadRequest.with_message("Pipeline schedule cron length is invalid")
                }
                WorkspacePipelineConstraints::ScheduleRequiresCron => ErrorKind::BadRequest
                    .with_message("A scheduled pipeline requires a cron expression"),
                WorkspacePipelineConstraints::ScheduleTzLength => ErrorKind::BadRequest
                    .with_message("Pipeline schedule timezone length is invalid"),
                WorkspacePipelineConstraints::SlugLength => ErrorKind::BadRequest
                    .with_message("Pipeline slug must be between 3 and 32 characters long"),
                WorkspacePipelineConstraints::SlugFormat => ErrorKind::BadRequest.with_message(
                    "Pipeline slug must be lowercase alphanumeric with single internal dashes",
                ),
                WorkspacePipelineConstraints::SlugUnique => {
                    ErrorKind::Conflict.with_message("A pipeline with this slug already exists")
                }
                WorkspacePipelineConstraints::WorkspaceIdIdUnique => ErrorKind::Conflict
                    .with_message("A pipeline with this identifier already exists"),
                WorkspacePipelineConstraints::UpdatedAfterCreated
                | WorkspacePipelineConstraints::DeletedAfterCreated => {
                    ErrorKind::InternalServerError.into_error()
                }
            };

        error.with_resource("pipeline")
    }
}

impl From<WorkspacePipelineRunConstraints> for Error<'static> {
    fn from(c: WorkspacePipelineRunConstraints) -> Self {
        let error =
            match c {
                WorkspacePipelineRunConstraints::AnalyzedDocumentKeyLength => {
                    ErrorKind::InternalServerError.into_error()
                }
                WorkspacePipelineRunConstraints::MetadataSize => ErrorKind::BadRequest
                    .with_message("Pipeline run metadata size exceeds maximum limit"),
                WorkspacePipelineRunConstraints::IdempotencyKeyLength => ErrorKind::BadRequest
                    .with_message("Idempotency key must be 1 to 255 characters"),
                WorkspacePipelineRunConstraints::IdempotencyUnique => ErrorKind::Conflict
                    .with_message("A run with this idempotency key already exists"),
                WorkspacePipelineRunConstraints::CompletedAfterStarted => {
                    ErrorKind::InternalServerError.into_error()
                }
            };

        error.with_resource("pipeline_run")
    }
}

impl From<WorkspacePipelineArtifactConstraints> for Error<'static> {
    fn from(c: WorkspacePipelineArtifactConstraints) -> Self {
        let error = match c {
            WorkspacePipelineArtifactConstraints::MetadataSize => {
                ErrorKind::BadRequest.with_message("Artifact metadata size exceeds maximum limit")
            }
        };

        error.with_resource("pipeline_artifact")
    }
}

impl From<WorkspacePipelineReferenceConstraints> for Error<'static> {
    fn from(c: WorkspacePipelineReferenceConstraints) -> Self {
        let (resource, error) = match c {
            WorkspacePipelineReferenceConstraints::PolicyReference => (
                "policy",
                ErrorKind::BadRequest
                    .with_message("Referenced policy does not exist in this workspace"),
            ),
            // The pipeline side of the FK only fails if the pipeline row vanished
            // mid-transaction, which is a server-side fault rather than bad input.
            WorkspacePipelineReferenceConstraints::PolicyPipelineReference => {
                ("pipeline", ErrorKind::InternalServerError.into_error())
            }
        };

        error.with_resource(resource)
    }
}

impl From<WorkspaceConnectionConstraints> for Error<'static> {
    fn from(c: WorkspaceConnectionConstraints) -> Self {
        let error =
            match c {
                WorkspaceConnectionConstraints::NameLength => ErrorKind::BadRequest
                    .with_message("Connection name must be between 1 and 255 characters"),
                WorkspaceConnectionConstraints::ProviderLength => ErrorKind::BadRequest
                    .with_message("Provider name must be between 1 and 64 characters"),
                WorkspaceConnectionConstraints::DataSize => {
                    ErrorKind::BadRequest.with_message("Connection data size exceeds maximum limit")
                }
                WorkspaceConnectionConstraints::MetadataSize => ErrorKind::BadRequest
                    .with_message("Connection metadata size exceeds maximum limit"),
                WorkspaceConnectionConstraints::ScheduleCronLength => {
                    ErrorKind::BadRequest.with_message("Connection schedule cron length is invalid")
                }
                WorkspaceConnectionConstraints::ScheduleImportOnly => {
                    ErrorKind::BadRequest.with_message("Only import connections can be scheduled")
                }
                WorkspaceConnectionConstraints::WorkspaceIdIdUnique => ErrorKind::Conflict
                    .with_message("A connection with this identifier already exists"),
                WorkspaceConnectionConstraints::NameUnique => {
                    ErrorKind::Conflict.with_message("A connection with this name already exists")
                }
                WorkspaceConnectionConstraints::UpdatedAfterCreated
                | WorkspaceConnectionConstraints::DeletedAfterCreated => {
                    ErrorKind::InternalServerError.into_error()
                }
            };

        error.with_resource("workspace_connection")
    }
}

impl From<WorkspaceConnectionSyncConstraints> for Error<'static> {
    fn from(c: WorkspaceConnectionSyncConstraints) -> Self {
        let error = match c {
            WorkspaceConnectionSyncConstraints::ErrorMessageLength => ErrorKind::BadRequest
                .with_message("Sync error message must be between 1 and 4096 characters"),
            WorkspaceConnectionSyncConstraints::MetadataSize => {
                ErrorKind::BadRequest.with_message("Sync run metadata size exceeds maximum limit")
            }
            WorkspaceConnectionSyncConstraints::OneActivePerConnection => {
                ErrorKind::Conflict.with_message("A sync is already in progress")
            }
            WorkspaceConnectionSyncConstraints::RecordsSyncedNonNegative
            | WorkspaceConnectionSyncConstraints::AttemptPositive
            | WorkspaceConnectionSyncConstraints::CompletedAfterStarted => {
                ErrorKind::InternalServerError.into_error()
            }
        };

        error.with_resource("workspace_connection_sync")
    }
}

impl From<WorkspacePolicyConstraints> for Error<'static> {
    fn from(c: WorkspacePolicyConstraints) -> Self {
        let error = match c {
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
            WorkspacePolicyConstraints::SlugLength => ErrorKind::BadRequest
                .with_message("Policy slug must be between 3 and 32 characters long"),
            WorkspacePolicyConstraints::SlugFormat => ErrorKind::BadRequest.with_message(
                "Policy slug must be lowercase alphanumeric with single internal dashes",
            ),
            WorkspacePolicyConstraints::SlugUnique => {
                ErrorKind::Conflict.with_message("A policy with this slug already exists")
            }
            WorkspacePolicyConstraints::NameUnique => {
                ErrorKind::Conflict.with_message("A policy with this name already exists")
            }
            WorkspacePolicyConstraints::WorkspaceIdIdUnique => {
                ErrorKind::Conflict.with_message("A policy with this identifier already exists")
            }
            WorkspacePolicyConstraints::UpdatedAfterCreated
            | WorkspacePolicyConstraints::DeletedAfterCreated => {
                ErrorKind::InternalServerError.into_error()
            }
        };

        error.with_resource("workspace_policy")
    }
}
