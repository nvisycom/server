//! Pipeline artifacts repository for managing pipeline run artifacts.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::model::{NewPipelineArtifact, PipelineArtifact};
use crate::types::ArtifactType;
use crate::{PgConnection, PgError, PgResult, schema};

/// Repository for pipeline artifact database operations.
///
/// Handles artifact lifecycle management including creation and queries
/// for pipeline run inputs, outputs, and intermediate artifacts.
pub trait PipelineArtifactRepository {
    /// Creates a new pipeline artifact record.
    fn create_pipeline_artifact(
        &mut self,
        new_artifact: NewPipelineArtifact,
    ) -> impl Future<Output = PgResult<PipelineArtifact>> + Send;

    /// Creates multiple pipeline artifacts in a batch.
    fn create_pipeline_artifacts(
        &mut self,
        new_artifacts: Vec<NewPipelineArtifact>,
    ) -> impl Future<Output = PgResult<Vec<PipelineArtifact>>> + Send;

    /// Finds an artifact by its unique identifier.
    fn find_pipeline_artifact_by_id(
        &mut self,
        artifact_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<PipelineArtifact>>> + Send;

    /// Finds an artifact by its file ID.
    fn find_artifact_by_file_id(
        &mut self,
        file_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<PipelineArtifact>>> + Send;

    /// Lists all artifacts for a pipeline run.
    fn list_run_artifacts(
        &mut self,
        run_id: Uuid,
    ) -> impl Future<Output = PgResult<Vec<PipelineArtifact>>> + Send;

    /// Lists artifacts for a pipeline run filtered by type.
    fn list_run_artifacts_by_type(
        &mut self,
        run_id: Uuid,
        artifact_type: ArtifactType,
    ) -> impl Future<Output = PgResult<Vec<PipelineArtifact>>> + Send;

    /// Lists input artifacts for a pipeline run.
    fn list_run_input_artifacts(
        &mut self,
        run_id: Uuid,
    ) -> impl Future<Output = PgResult<Vec<PipelineArtifact>>> + Send;

    /// Lists output artifacts for a pipeline run.
    fn list_run_output_artifacts(
        &mut self,
        run_id: Uuid,
    ) -> impl Future<Output = PgResult<Vec<PipelineArtifact>>> + Send;

    /// Deletes all artifacts for a pipeline run.
    fn delete_run_artifacts(&mut self, run_id: Uuid) -> impl Future<Output = PgResult<u64>> + Send;

    /// Counts artifacts for a pipeline run.
    fn count_run_artifacts(&mut self, run_id: Uuid) -> impl Future<Output = PgResult<i64>> + Send;

    /// Lists all artifacts for a pipeline (across all runs).
    fn list_pipeline_artifacts(
        &mut self,
        pipeline_id: Uuid,
    ) -> impl Future<Output = PgResult<Vec<PipelineArtifact>>> + Send;
}

impl PipelineArtifactRepository for PgConnection {
    async fn create_pipeline_artifact(
        &mut self,
        new_artifact: NewPipelineArtifact,
    ) -> PgResult<PipelineArtifact> {
        use schema::pipeline_artifacts;

        let artifact = diesel::insert_into(pipeline_artifacts::table)
            .values(&new_artifact)
            .returning(PipelineArtifact::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(artifact)
    }

    async fn create_pipeline_artifacts(
        &mut self,
        new_artifacts: Vec<NewPipelineArtifact>,
    ) -> PgResult<Vec<PipelineArtifact>> {
        use schema::pipeline_artifacts;

        let artifacts = diesel::insert_into(pipeline_artifacts::table)
            .values(&new_artifacts)
            .returning(PipelineArtifact::as_returning())
            .get_results(self)
            .await
            .map_err(PgError::from)?;

        Ok(artifacts)
    }

    async fn find_pipeline_artifact_by_id(
        &mut self,
        artifact_id: Uuid,
    ) -> PgResult<Option<PipelineArtifact>> {
        use schema::pipeline_artifacts::{self, dsl};

        let artifact = pipeline_artifacts::table
            .filter(dsl::id.eq(artifact_id))
            .select(PipelineArtifact::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(artifact)
    }

    async fn find_artifact_by_file_id(
        &mut self,
        file_id: Uuid,
    ) -> PgResult<Option<PipelineArtifact>> {
        use schema::pipeline_artifacts::{self, dsl};

        let artifact = pipeline_artifacts::table
            .filter(dsl::file_id.eq(file_id))
            .select(PipelineArtifact::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(artifact)
    }

    async fn list_run_artifacts(&mut self, run_id: Uuid) -> PgResult<Vec<PipelineArtifact>> {
        use schema::pipeline_artifacts::{self, dsl};

        let artifacts = pipeline_artifacts::table
            .filter(dsl::run_id.eq(run_id))
            .order(dsl::created_at.asc())
            .select(PipelineArtifact::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(artifacts)
    }

    async fn list_run_artifacts_by_type(
        &mut self,
        run_id: Uuid,
        artifact_type: ArtifactType,
    ) -> PgResult<Vec<PipelineArtifact>> {
        use schema::pipeline_artifacts::{self, dsl};

        let artifacts = pipeline_artifacts::table
            .filter(dsl::run_id.eq(run_id))
            .filter(dsl::artifact_type.eq(artifact_type))
            .order(dsl::created_at.asc())
            .select(PipelineArtifact::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(artifacts)
    }

    async fn list_run_input_artifacts(&mut self, run_id: Uuid) -> PgResult<Vec<PipelineArtifact>> {
        self.list_run_artifacts_by_type(run_id, ArtifactType::Input)
            .await
    }

    async fn list_run_output_artifacts(&mut self, run_id: Uuid) -> PgResult<Vec<PipelineArtifact>> {
        self.list_run_artifacts_by_type(run_id, ArtifactType::Output)
            .await
    }

    async fn delete_run_artifacts(&mut self, run_id: Uuid) -> PgResult<u64> {
        use schema::pipeline_artifacts::{self, dsl};

        let deleted = diesel::delete(pipeline_artifacts::table.filter(dsl::run_id.eq(run_id)))
            .execute(self)
            .await
            .map_err(PgError::from)?;

        Ok(deleted as u64)
    }

    async fn count_run_artifacts(&mut self, run_id: Uuid) -> PgResult<i64> {
        use schema::pipeline_artifacts::{self, dsl};

        let count = pipeline_artifacts::table
            .filter(dsl::run_id.eq(run_id))
            .count()
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(count)
    }

    async fn list_pipeline_artifacts(
        &mut self,
        pipeline_id: Uuid,
    ) -> PgResult<Vec<PipelineArtifact>> {
        use schema::{pipeline_artifacts, pipeline_runs};

        let artifacts = pipeline_artifacts::table
            .inner_join(pipeline_runs::table)
            .filter(pipeline_runs::pipeline_id.eq(pipeline_id))
            .select(PipelineArtifact::as_select())
            .order(pipeline_artifacts::created_at.desc())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(artifacts)
    }
}
