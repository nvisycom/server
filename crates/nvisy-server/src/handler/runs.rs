//! Pipeline run handlers: detect, review, and redact.
//!
//! A run is one analysis of a file through a pipeline. Detect creates the run
//! and stores the findings; the run then awaits reviewer verification before
//! redact consumes the verified findings and produces a redacted file.

use std::io::Cursor;
use std::str::FromStr;

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use bytes::Bytes;
use nvisy_engine::AnalyzedDocument;
use nvisy_nats::NatsClient;
use nvisy_nats::object::{FileKey, FilesBucket, IntermediateKey, IntermediatesBucket};
use nvisy_postgres::model::{
    NewWorkspaceFile, NewWorkspacePipelineArtifact, NewWorkspacePipelineRun,
    UpdateWorkspacePipelineRun, WorkspaceFile, WorkspacePipelineArtifact, WorkspacePipelineRun,
};
use nvisy_postgres::query::{
    PipelineReferenceRepository, WorkspaceContextRepository, WorkspaceFileRepository,
    WorkspacePipelineArtifactRepository, WorkspacePipelineRepository,
    WorkspacePipelineRunRepository, WorkspacePolicyRepository,
};
use nvisy_postgres::types::{ArtifactType, PipelineRunStatus};
use nvisy_postgres::{PgClient, PgConn};
use nvisy_schema::context::Context as SchemaContext;
use nvisy_schema::file::Document;
use nvisy_schema::plan::{AnalyzerParams, ScopeParams};
use nvisy_schema::policy::Policy as SchemaPolicy;
use sha2::{Digest, Sha256};
use tokio::io::AsyncReadExt;
use uuid::Uuid;

use crate::extract::{AuthProvider, AuthState, Json, Path, Permission, Query, ValidateJson};
use crate::handler::request::{
    CreatePipelineRun, CursorPagination, PipelineDefinition, PipelinePathParams,
    PipelineRunPathParams,
};
use crate::handler::response::{ErrorResponse, PipelineRun, PipelineRunsPage};
use crate::handler::{Error, ErrorKind, Result};
use crate::service::{CryptoService, EngineService, ServiceState};

/// Tracing target for pipeline run operations.
const TRACING_TARGET: &str = "nvisy_server::handler::runs";

/// Header carrying the detect idempotency key.
const IDEMPOTENCY_HEADER: &str = "idempotency-key";

/// Starts a run: analyzes a file with the pipeline's configuration (detect).
///
/// Returns the run holding the findings for review. A repeated request with the
/// same `Idempotency-Key` returns the existing run instead of analyzing again.
/// Requires `RunPipelines` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        pipeline_id = %path_params.pipeline_id,
    )
)]
async fn create_pipeline_run(
    State(pg_client): State<PgClient>,
    State(nats): State<NatsClient>,
    State(crypto): State<CryptoService>,
    State(engine): State<EngineService>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<PipelinePathParams>,
    headers: HeaderMap,
    ValidateJson(request): ValidateJson<CreatePipelineRun>,
) -> Result<(StatusCode, Json<PipelineRun>)> {
    tracing::debug!(target: TRACING_TARGET, "Starting pipeline run (detect)");

    let mut conn = pg_client.get_connection().await?;

    let pipeline = conn
        .find_workspace_pipeline_by_id(path_params.pipeline_id)
        .await?
        .ok_or_else(|| Error::not_found("pipeline"))?;

    auth_state
        .authorize_workspace(&mut conn, pipeline.workspace_id, Permission::RunPipelines)
        .await?;

    let idempotency_key = idempotency_key(&headers)?;

    // Idempotent replay: a repeated key returns the run created the first time.
    if let Some(key) = &idempotency_key
        && let Some(existing) = conn
            .find_pipeline_run_by_idempotency_key(path_params.pipeline_id, key)
            .await?
    {
        tracing::debug!(target: TRACING_TARGET, "Replaying run for idempotency key");
        return Ok((StatusCode::OK, Json(PipelineRun::from_model(existing))));
    }

    let file = conn
        .find_file_in_workspace(pipeline.workspace_id, request.file_id)
        .await?
        .ok_or_else(|| Error::not_found("file"))?;

    let definition = PipelineDefinition::from_parts(pipeline.definition, Vec::new(), Vec::new())
        .map_err(serialize_error)?;

    // Create the run first so its id is the engine correlation id.
    let new_run = NewWorkspacePipelineRun {
        pipeline_id: pipeline.id,
        file_id: file.id,
        account_id: Some(auth_state.account_id),
        status: Some(PipelineRunStatus::Running),
        idempotency_key: idempotency_key.clone(),
        ..Default::default()
    };
    let run = conn.create_workspace_pipeline_run(new_run).await?;

    // Assemble the engine inputs and analyze.
    let document = build_document(&nats, &crypto, &file, run.id).await?;
    let params = build_analyzer_params(&definition, request.scope);
    let contexts = resolve_contexts(&mut conn, &crypto, pipeline.workspace_id, pipeline.id).await?;

    let analyzed = match engine.analyze_document(document, &params, &contexts).await {
        Ok(analyzed) => analyzed,
        Err(err) => {
            fail_run(&mut conn, run.id).await;
            return Err(analysis_error(err));
        }
    };

    // The analysis is a map of detected PII; encrypt it and hold it in the
    // intermediates bucket, keeping only its key on the run.
    let analyzed_key =
        store_analyzed_document(&nats, &crypto, pipeline.workspace_id, &analyzed).await?;
    let run = conn
        .update_workspace_pipeline_run(
            run.id,
            UpdateWorkspacePipelineRun {
                status: Some(PipelineRunStatus::Analyzed),
                analyzed_document_key: Some(Some(analyzed_key)),
                ..Default::default()
            },
        )
        .await?;

    tracing::info!(target: TRACING_TARGET, run_id = %run.id, "Pipeline run analyzed");

    Ok((StatusCode::CREATED, Json(PipelineRun::from_model(run))))
}

fn create_pipeline_run_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Start a run (detect)")
        .description(
            "Analyzes a file with the pipeline's configuration and returns the run \
             holding the findings for review. Accepts an Idempotency-Key header.",
        )
        .response::<201, Json<PipelineRun>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Lists runs for a specific pipeline.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        pipeline_id = %path_params.pipeline_id,
    )
)]
async fn list_pipeline_runs(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<PipelinePathParams>,
    Query(pagination): Query<CursorPagination>,
) -> Result<(StatusCode, Json<PipelineRunsPage>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing pipeline runs");

    let mut conn = pg_client.get_connection().await?;

    let Some(pipeline) = conn
        .find_workspace_pipeline_by_id(path_params.pipeline_id)
        .await?
    else {
        return Err(ErrorKind::NotFound
            .with_message("Pipeline not found")
            .with_resource("pipeline"));
    };

    auth_state
        .authorize_workspace(&mut conn, pipeline.workspace_id, Permission::ViewPipelines)
        .await?;

    let page = conn
        .cursor_list_workspace_pipeline_runs(path_params.pipeline_id, pagination.into(), None)
        .await?;

    tracing::debug!(
        target: TRACING_TARGET,
        run_count = page.items.len(),
        "Pipeline runs listed"
    );

    Ok((
        StatusCode::OK,
        Json(PipelineRunsPage::from_cursor_page(
            page,
            PipelineRun::from_model,
        )),
    ))
}

fn list_pipeline_runs_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List pipeline runs")
        .description("Returns all runs for a specific pipeline.")
        .response::<200, Json<PipelineRunsPage>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Gets a specific pipeline run.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        run_id = %path_params.run_id,
    )
)]
async fn get_pipeline_run(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<PipelineRunPathParams>,
) -> Result<(StatusCode, Json<PipelineRun>)> {
    tracing::debug!(target: TRACING_TARGET, "Getting pipeline run");

    let mut conn = pg_client.get_connection().await?;

    let run = conn
        .find_workspace_pipeline_run_by_id(path_params.run_id)
        .await?
        .ok_or_else(|| {
            ErrorKind::NotFound
                .with_message("Pipeline run not found")
                .with_resource("pipeline_run")
        })?;

    // Get workspace_id from the pipeline
    let pipeline = conn
        .find_workspace_pipeline_by_id(run.pipeline_id)
        .await?
        .ok_or_else(|| {
            ErrorKind::NotFound
                .with_message("Pipeline not found")
                .with_resource("pipeline")
        })?;

    auth_state
        .authorize_workspace(&mut conn, pipeline.workspace_id, Permission::ViewPipelines)
        .await?;

    tracing::debug!(target: TRACING_TARGET, "Pipeline run retrieved");

    Ok((StatusCode::OK, Json(PipelineRun::from_model(run))))
}

fn get_pipeline_run_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get pipeline run")
        .description("Returns the run and its status for review.")
        .response::<200, Json<PipelineRun>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Returns the run's analyzed document (the detected findings) for review.
///
/// Fetches and decrypts the engine's `AnalyzedDocument` from the intermediates
/// bucket. Available once the run is analyzed. Requires `ViewPipelines`.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        run_id = %path_params.run_id,
    )
)]
async fn get_pipeline_run_analysis(
    State(pg_client): State<PgClient>,
    State(nats): State<NatsClient>,
    State(crypto): State<CryptoService>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<PipelineRunPathParams>,
) -> Result<(StatusCode, Json<AnalyzedDocument>)> {
    tracing::debug!(target: TRACING_TARGET, "Getting pipeline run analysis");

    let mut conn = pg_client.get_connection().await?;

    let run = conn
        .find_workspace_pipeline_run_by_id(path_params.run_id)
        .await?
        .ok_or_else(|| Error::not_found("pipeline_run"))?;

    let pipeline = conn
        .find_workspace_pipeline_by_id(run.pipeline_id)
        .await?
        .ok_or_else(|| Error::not_found("pipeline"))?;

    auth_state
        .authorize_workspace(&mut conn, pipeline.workspace_id, Permission::ViewPipelines)
        .await?;

    let analyzed = load_analyzed_document(&nats, &crypto, pipeline.workspace_id, &run).await?;

    tracing::debug!(target: TRACING_TARGET, "Pipeline run analysis retrieved");

    Ok((StatusCode::OK, Json(analyzed)))
}

fn get_pipeline_run_analysis_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get run analysis")
        .description("Returns the run's detected findings (the analyzed document) for review.")
        .response::<200, Json<AnalyzedDocument>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
        .response::<409, Json<ErrorResponse>>()
}

/// Redacts a run using the reviewer-verified findings, storing the result.
///
/// Consumes the analyzed run (which must be awaiting review), applies the
/// pipeline's policies to the verified findings, stores the redacted bytes as a
/// new file, and completes the run. Requires `RunPipelines` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        run_id = %path_params.run_id,
    )
)]
async fn redact_pipeline_run(
    State(pg_client): State<PgClient>,
    State(nats): State<NatsClient>,
    State(crypto): State<CryptoService>,
    State(engine): State<EngineService>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<PipelineRunPathParams>,
) -> Result<(StatusCode, Json<PipelineRun>)> {
    tracing::debug!(target: TRACING_TARGET, "Redacting pipeline run");

    let mut conn = pg_client.get_connection().await?;

    let run = conn
        .find_workspace_pipeline_run_by_id(path_params.run_id)
        .await?
        .ok_or_else(|| Error::not_found("pipeline_run"))?;

    let pipeline = conn
        .find_workspace_pipeline_by_id(run.pipeline_id)
        .await?
        .ok_or_else(|| Error::not_found("pipeline"))?;

    auth_state
        .authorize_workspace(&mut conn, pipeline.workspace_id, Permission::RunPipelines)
        .await?;

    // A run can only be redacted once, after detection.
    if !run.is_analyzed() {
        return Err(ErrorKind::Conflict
            .with_message("Run is not awaiting redaction")
            .with_resource("pipeline_run"));
    }

    let file = conn
        .find_file_in_workspace(pipeline.workspace_id, run.file_id)
        .await?
        .ok_or_else(|| Error::not_found("file"))?;

    // The stored analysis is the source of truth for what gets redacted.
    let analyzed = load_analyzed_document(&nats, &crypto, pipeline.workspace_id, &run).await?;
    let policies = resolve_policies(&mut conn, &crypto, pipeline.workspace_id, pipeline.id).await?;
    let document = build_document(&nats, &crypto, &file, run.id).await?;

    let anonymized = engine
        .anonymize_document(document, &policies, &analyzed)
        .await
        .map_err(analysis_error)?;

    // Store the redacted bytes as a new workspace file and record the artifact.
    let artifact_file = store_redacted_file(
        &mut conn,
        &nats,
        &crypto,
        &file,
        auth_state.account_id,
        anonymized.bytes,
    )
    .await?;
    record_artifact(&mut conn, run.id, artifact_file.id).await?;

    let run = conn
        .update_workspace_pipeline_run(
            run.id,
            UpdateWorkspacePipelineRun {
                status: Some(PipelineRunStatus::Completed),
                completed_at: Some(Some(jiff::Timestamp::now().into())),
                ..Default::default()
            },
        )
        .await?;

    tracing::info!(
        target: TRACING_TARGET,
        run_id = %run.id,
        artifact_file_id = %artifact_file.id,
        "Pipeline run redacted"
    );

    Ok((StatusCode::OK, Json(PipelineRun::from_model(run))))
}

fn redact_pipeline_run_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Redact a run")
        .description(
            "Applies the pipeline's policies to the run's stored analysis, stores \
             the redacted file, and completes the run.",
        )
        .response::<200, Json<PipelineRun>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
        .response::<409, Json<ErrorResponse>>()
}

/// Extracts and validates the optional idempotency key header.
fn idempotency_key(headers: &HeaderMap) -> Result<Option<String>> {
    let Some(value) = headers.get(IDEMPOTENCY_HEADER) else {
        return Ok(None);
    };
    let key = value.to_str().map_err(|_| {
        ErrorKind::BadRequest.with_message("Idempotency-Key must be a valid ASCII string")
    })?;
    if key.is_empty() || key.len() > 255 {
        return Err(
            ErrorKind::BadRequest.with_message("Idempotency-Key must be 1 to 255 characters")
        );
    }
    Ok(Some(key.to_owned()))
}

/// Marks a run failed (best effort) after an engine error.
async fn fail_run(conn: &mut PgConn, run_id: uuid::Uuid) {
    let update = UpdateWorkspacePipelineRun {
        status: Some(PipelineRunStatus::Failed),
        completed_at: Some(Some(jiff::Timestamp::now().into())),
        ..Default::default()
    };
    if let Err(err) = conn.update_workspace_pipeline_run(run_id, update).await {
        tracing::warn!(target: TRACING_TARGET, error = %err, "Failed to mark run failed");
    }
}

/// Maps a definition (de)serialization failure to an internal error.
fn serialize_error(error: serde_json::Error) -> Error<'static> {
    ErrorKind::InternalServerError
        .with_message("Failed to process pipeline definition")
        .with_context(error.to_string())
}

/// Maps an engine analyze/anonymize failure to an internal error.
fn analysis_error(error: nvisy_engine::Error) -> Error<'static> {
    ErrorKind::InternalServerError
        .with_message("Redaction engine failed")
        .with_context(error.to_string())
}

/// Returns a [`Router`] with all pipeline run routes.
///
/// [`Router`]: axum::routing::Router
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route(
            "/pipelines/{pipelineId}/runs/",
            post_with(create_pipeline_run, create_pipeline_run_docs)
                .get_with(list_pipeline_runs, list_pipeline_runs_docs),
        )
        .api_route(
            "/runs/{runId}/",
            get_with(get_pipeline_run, get_pipeline_run_docs),
        )
        .api_route(
            "/runs/{runId}/analysis",
            get_with(get_pipeline_run_analysis, get_pipeline_run_analysis_docs),
        )
        .api_route(
            "/runs/{runId}/redact/",
            post_with(redact_pipeline_run, redact_pipeline_run_docs),
        )
        .with_path_items(|item| item.tag("Pipeline Runs"))
}

/// Reads a workspace file's bytes from object storage and builds an engine
/// [`Document`], stamping the run's id as the correlation id.
async fn build_document(
    nats: &NatsClient,
    crypto: &CryptoService,
    file: &WorkspaceFile,
    correlation_id: Uuid,
) -> Result<Document> {
    let store = nats.object_store::<FilesBucket, FileKey>().await?;
    let key = FileKey::from_str(&file.storage_path).map_err(|err| {
        ErrorKind::InternalServerError
            .with_message("Invalid file storage path")
            .with_context(err.to_string())
    })?;

    let data = store.get(&key).await?.ok_or_else(|| {
        ErrorKind::InternalServerError.with_message("File content is missing from storage")
    })?;
    let mut reader = data.into_reader();
    let mut ciphertext = Vec::new();
    reader.read_to_end(&mut ciphertext).await.map_err(|err| {
        ErrorKind::InternalServerError
            .with_message("Failed to read file content")
            .with_context(err.to_string())
    })?;

    let bytes = crypto
        .decrypt(file.workspace_id, &ciphertext)
        .map_err(|err| {
            ErrorKind::InternalServerError
                .with_message("Failed to decrypt file content")
                .with_context(err.to_string())
        })?;

    Ok(Document::new(bytes, file.file_extension.clone()).with_correlation_id(correlation_id))
}

/// Assembles the engine's [`AnalyzerParams`] for one detect request.
///
/// The recognizers, enrichers, and deduplication come from the pipeline's
/// stored config; the scope is the request's own (falling back to the pipeline
/// default), with the pipeline's reusable label catalog folded in.
fn build_analyzer_params(
    definition: &PipelineDefinition,
    request_scope: Option<ScopeParams>,
) -> AnalyzerParams {
    let mut scope = request_scope
        .or_else(|| definition.default_scope.clone())
        .unwrap_or_default();
    scope.label_catalog = definition.label_catalog.clone();

    AnalyzerParams {
        recognizers: definition.recognizers.clone(),
        enrichers: definition.enrichers.clone(),
        deduplication: definition.deduplication.clone(),
        scope,
    }
}

/// Resolves a pipeline's live context references into decrypted engine contexts.
///
/// Soft-deleted contexts are already filtered out by the repository.
async fn resolve_contexts(
    conn: &mut PgConn,
    crypto: &CryptoService,
    workspace_id: Uuid,
    pipeline_id: Uuid,
) -> Result<Vec<SchemaContext>> {
    let ids = conn.list_pipeline_context_ids(pipeline_id).await?;
    let mut contexts = Vec::with_capacity(ids.len());
    for id in ids {
        if let Some(model) = conn.find_context_in_workspace(workspace_id, id).await? {
            contexts.push(crypto.decrypt_json::<SchemaContext>(workspace_id, &model.definition)?);
        }
    }
    Ok(contexts)
}

/// Resolves a pipeline's live policy references into decrypted engine policies.
async fn resolve_policies(
    conn: &mut PgConn,
    crypto: &CryptoService,
    workspace_id: Uuid,
    pipeline_id: Uuid,
) -> Result<Vec<SchemaPolicy>> {
    let ids = conn.list_pipeline_policy_ids(pipeline_id).await?;
    let mut policies = Vec::with_capacity(ids.len());
    for id in ids {
        if let Some(model) = conn.find_policy_in_workspace(workspace_id, id).await? {
            policies.push(crypto.decrypt_json::<SchemaPolicy>(workspace_id, &model.definition)?);
        }
    }
    Ok(policies)
}

/// Stores redacted bytes as a new workspace file (the run's output).
///
/// The redacted file is a first-class file — a sibling of the source — so it is
/// downloadable through the normal file endpoints.
async fn store_redacted_file(
    conn: &mut PgConn,
    nats: &NatsClient,
    crypto: &CryptoService,
    source: &WorkspaceFile,
    account_id: Uuid,
    bytes: Bytes,
) -> Result<WorkspaceFile> {
    // Record the plaintext size and hash before encrypting; storage holds only
    // the ciphertext.
    let plaintext_size = bytes.len() as i64;
    let plaintext_hash = Sha256::digest(&bytes).to_vec();
    let ciphertext = crypto.encrypt(source.workspace_id, &bytes).map_err(|err| {
        ErrorKind::InternalServerError
            .with_message("Failed to encrypt redacted file")
            .with_context(err.to_string())
    })?;

    let store = nats.object_store::<FilesBucket, FileKey>().await?;
    let key = FileKey::generate(source.workspace_id);
    store.put(&key, Cursor::new(ciphertext)).await?;

    let redacted_name = format!("{}.redacted", source.display_name);
    let new_file = NewWorkspaceFile {
        workspace_id: source.workspace_id,
        account_id,
        parent_id: Some(source.id),
        display_name: Some(redacted_name),
        original_filename: Some(source.original_filename.clone()),
        file_extension: Some(source.file_extension.clone()),
        mime_type: source.mime_type.clone(),
        file_size_bytes: plaintext_size,
        file_hash_sha256: plaintext_hash,
        storage_path: key.to_string(),
        storage_bucket: store.bucket().to_owned(),
        ..Default::default()
    };

    Ok(conn.create_workspace_file(new_file).await?)
}

/// Encrypts an [`AnalyzedDocument`] and stores it in the intermediates bucket,
/// returning its object-store key.
///
/// The analysis is the map of detected PII, so it is encrypted with the
/// workspace key before it leaves the process.
async fn store_analyzed_document(
    nats: &NatsClient,
    crypto: &CryptoService,
    workspace_id: Uuid,
    analyzed: &AnalyzedDocument,
) -> Result<String> {
    let plaintext = serde_json::to_vec(analyzed).map_err(serialize_error)?;
    let ciphertext = crypto.encrypt(workspace_id, &plaintext).map_err(|err| {
        ErrorKind::InternalServerError
            .with_message("Failed to encrypt analysis")
            .with_context(err.to_string())
    })?;

    let store = nats
        .object_store::<IntermediatesBucket, IntermediateKey>()
        .await?;
    let key = IntermediateKey::generate(workspace_id);
    store.put(&key, Cursor::new(ciphertext)).await?;

    Ok(key.to_string())
}

/// Fetches and decrypts a run's stored [`AnalyzedDocument`].
///
/// Errors if the run has not been analyzed yet or the stored object is missing.
async fn load_analyzed_document(
    nats: &NatsClient,
    crypto: &CryptoService,
    workspace_id: Uuid,
    run: &WorkspacePipelineRun,
) -> Result<AnalyzedDocument> {
    let stored_key = run.analyzed_document_key.as_deref().ok_or_else(|| {
        ErrorKind::Conflict
            .with_message("Run has no analysis yet")
            .with_resource("pipeline_run")
    })?;
    let key = IntermediateKey::from_str(stored_key).map_err(|err| {
        ErrorKind::InternalServerError
            .with_message("Invalid analysis storage key")
            .with_context(err.to_string())
    })?;

    let store = nats
        .object_store::<IntermediatesBucket, IntermediateKey>()
        .await?;
    let data = store.get(&key).await?.ok_or_else(|| {
        ErrorKind::InternalServerError.with_message("Analysis is missing from storage")
    })?;
    let mut reader = data.into_reader();
    let mut ciphertext = Vec::new();
    reader.read_to_end(&mut ciphertext).await.map_err(|err| {
        ErrorKind::InternalServerError
            .with_message("Failed to read analysis")
            .with_context(err.to_string())
    })?;

    let plaintext = crypto.decrypt(workspace_id, &ciphertext).map_err(|err| {
        ErrorKind::InternalServerError
            .with_message("Failed to decrypt analysis")
            .with_context(err.to_string())
    })?;
    serde_json::from_slice(&plaintext).map_err(serialize_error)
}

/// Records that a run produced an output file (the redaction artifact).
async fn record_artifact(
    conn: &mut PgConn,
    run_id: Uuid,
    file_id: Uuid,
) -> Result<WorkspacePipelineArtifact> {
    let artifact = NewWorkspacePipelineArtifact {
        run_id,
        file_id,
        artifact_type: ArtifactType::Output,
        metadata: None,
    };
    Ok(conn.create_workspace_pipeline_artifact(artifact).await?)
}
