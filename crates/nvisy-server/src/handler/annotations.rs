//! Document annotation handlers.
//!
//! This module provides CRUD handlers for document annotations.

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::http::StatusCode;
use nvisy_postgres::PgClient;
use nvisy_postgres::query::{WorkspaceFileAnnotationRepository, WorkspaceFileRepository};

use crate::extract::{AuthProvider, AuthState, Json, Path, Permission, Query, ValidateJson};
use crate::handler::request::{
    AnnotationPathParams, CreateAnnotation, CursorPagination, FilePathParams, UpdateAnnotation,
};
use crate::handler::response::{Annotation, AnnotationsPage, ErrorResponse};
use crate::handler::{ErrorKind, Result};
use crate::service::ServiceState;

/// Tracing target for annotation operations.
const TRACING_TARGET: &str = "nvisy_server::handler::annotations";

/// Finds an annotation by ID or returns NotFound error.
async fn find_annotation(
    conn: &mut nvisy_postgres::PgConn,
    annotation_id: uuid::Uuid,
) -> Result<nvisy_postgres::model::WorkspaceFileAnnotation> {
    conn.find_workspace_file_annotation_by_id(annotation_id)
        .await?
        .ok_or_else(|| {
            ErrorKind::NotFound
                .with_message("Annotation not found")
                .with_resource("annotation")
        })
}

/// Finds a file by ID or returns NotFound error.
async fn find_file(
    conn: &mut nvisy_postgres::PgConn,
    file_id: uuid::Uuid,
) -> Result<nvisy_postgres::model::WorkspaceFile> {
    conn.find_workspace_file_by_id(file_id)
        .await?
        .ok_or_else(|| {
            ErrorKind::NotFound
                .with_message("File not found")
                .with_resource("file")
        })
}

/// Creates a new annotation on a file.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        file_id = %path_params.file_id,
    )
)]
async fn create_annotation(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<FilePathParams>,
    ValidateJson(request): ValidateJson<CreateAnnotation>,
) -> Result<(StatusCode, Json<Annotation>)> {
    tracing::debug!(target: TRACING_TARGET, "Creating annotation");

    let mut conn = pg_client.get_connection().await?;
    let file = find_file(&mut conn, path_params.file_id).await?;

    auth_state
        .authorize_workspace(&mut conn, file.workspace_id, Permission::AnnotateFiles)
        .await?;

    let new_annotation = request.into_model(path_params.file_id, auth_state.account_id);
    let annotation = conn
        .create_workspace_file_annotation(new_annotation)
        .await?;

    tracing::info!(
        target: TRACING_TARGET,
        annotation_id = %annotation.id,
        "Annotation created"
    );

    Ok((
        StatusCode::CREATED,
        Json(Annotation::from_model(annotation)),
    ))
}

fn create_annotation_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Create annotation")
        .description("Creates a new annotation on a file.")
        .response::<201, Json<Annotation>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Lists annotations for a file.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        file_id = %path_params.file_id,
    )
)]
async fn list_annotations(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<FilePathParams>,
    Query(pagination): Query<CursorPagination>,
) -> Result<(StatusCode, Json<AnnotationsPage>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing annotations");

    let mut conn = pg_client.get_connection().await?;
    let file = find_file(&mut conn, path_params.file_id).await?;

    auth_state
        .authorize_workspace(&mut conn, file.workspace_id, Permission::ViewFiles)
        .await?;

    let page = conn
        .cursor_list_workspace_file_annotations(path_params.file_id, pagination.into())
        .await?;

    let response = AnnotationsPage::from_cursor_page(page, Annotation::from_model);

    tracing::debug!(
        target: TRACING_TARGET,
        annotation_count = response.items.len(),
        "Annotations listed"
    );

    Ok((StatusCode::OK, Json(response)))
}

fn list_annotations_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List annotations")
        .description("Returns all annotations for a file.")
        .response::<200, Json<AnnotationsPage>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Gets a specific annotation.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        annotation_id = %path_params.annotation_id,
    )
)]
async fn get_annotation(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<AnnotationPathParams>,
) -> Result<(StatusCode, Json<Annotation>)> {
    tracing::debug!(target: TRACING_TARGET, "Getting annotation");

    let mut conn = pg_client.get_connection().await?;
    let annotation = find_annotation(&mut conn, path_params.annotation_id).await?;
    let file = find_file(&mut conn, annotation.file_id).await?;

    auth_state
        .authorize_workspace(&mut conn, file.workspace_id, Permission::ViewFiles)
        .await?;

    tracing::debug!(target: TRACING_TARGET, "Annotation retrieved");

    Ok((StatusCode::OK, Json(Annotation::from_model(annotation))))
}

fn get_annotation_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get annotation")
        .description("Returns a specific annotation.")
        .response::<200, Json<Annotation>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Updates an annotation.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        annotation_id = %path_params.annotation_id,
    )
)]
async fn update_annotation(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<AnnotationPathParams>,
    ValidateJson(request): ValidateJson<UpdateAnnotation>,
) -> Result<(StatusCode, Json<Annotation>)> {
    tracing::debug!(target: TRACING_TARGET, "Updating annotation");

    let mut conn = pg_client.get_connection().await?;
    let annotation = find_annotation(&mut conn, path_params.annotation_id).await?;

    // Only the owner can update their annotation
    if annotation.account_id != auth_state.account_id {
        return Err(ErrorKind::Forbidden.with_message("You can only update your own annotations"));
    }

    let file = find_file(&mut conn, annotation.file_id).await?;

    auth_state
        .authorize_workspace(&mut conn, file.workspace_id, Permission::AnnotateFiles)
        .await?;

    let updated = conn
        .update_workspace_file_annotation(path_params.annotation_id, request.into_model())
        .await?;

    tracing::info!(target: TRACING_TARGET, "Annotation updated");

    Ok((StatusCode::OK, Json(Annotation::from_model(updated))))
}

fn update_annotation_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Update annotation")
        .description("Updates an annotation. Only the owner can update their annotations.")
        .response::<200, Json<Annotation>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Deletes an annotation.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        annotation_id = %path_params.annotation_id,
    )
)]
async fn delete_annotation(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<AnnotationPathParams>,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Deleting annotation");

    let mut conn = pg_client.get_connection().await?;
    let annotation = find_annotation(&mut conn, path_params.annotation_id).await?;

    // Only the owner can delete their annotation
    if annotation.account_id != auth_state.account_id {
        return Err(ErrorKind::Forbidden.with_message("You can only delete your own annotations"));
    }

    let file = find_file(&mut conn, annotation.file_id).await?;

    auth_state
        .authorize_workspace(&mut conn, file.workspace_id, Permission::AnnotateFiles)
        .await?;

    conn.delete_workspace_file_annotation(path_params.annotation_id)
        .await?;

    tracing::info!(target: TRACING_TARGET, "Annotation deleted");

    Ok(StatusCode::NO_CONTENT)
}

fn delete_annotation_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Delete annotation")
        .description("Deletes an annotation. Only the owner can delete their annotations.")
        .response::<204, ()>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Returns routes for annotation management.
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route(
            "/files/{fileId}/annotations/",
            post_with(create_annotation, create_annotation_docs)
                .get_with(list_annotations, list_annotations_docs),
        )
        .api_route(
            "/annotations/{annotationId}",
            get_with(get_annotation, get_annotation_docs)
                .patch_with(update_annotation, update_annotation_docs)
                .delete_with(delete_annotation, delete_annotation_docs),
        )
        .with_path_items(|item| item.tag("Annotations"))
}
