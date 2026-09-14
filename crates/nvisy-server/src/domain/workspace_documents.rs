//! Workspace document metadata domain logic: list, read, update, delete.
//!
//! Owns the document metadata rules — list-filter resolution against the engine's
//! codec registry, existence checks, and the delete that drops a document's blob
//! reference — in one place, factored out of the handler. The service holds the
//! engine (to resolve format and modality filter tokens). The byte-I/O actions
//! (upload, download) stay in the handler; they load a document through
//! [`find`](WorkspaceDocumentService::find).

use std::collections::BTreeSet;

use nvisy_postgres::model::WorkspaceDocument as DocumentModel;
use nvisy_postgres::query::{DocumentCursor, DocumentWithBlob, WorkspaceDocumentRepository};
use nvisy_postgres::types::{CursorPage, CursorPagination, WithAccountRef};
use nvisy_postgres::{AsyncConnection, PgClient, PgConn};
use uuid::Uuid;

use crate::domain::input::{ListDocumentsInput, UpdateDocumentInput};
use crate::domain::output::BulkDeleteOutcome;
use crate::response::{Error, ErrorKind, Result};
use crate::service::event::EventEmitter;
use crate::service::{EngineService, event};

/// Tracing target for document domain operations.
const TRACING_TARGET: &str = "nvisy_server::domain::document";

/// Lists, reads, updates, and deletes workspace document metadata.
///
/// Holds the Postgres client (acquiring its own connection per call) and the
/// engine (to resolve list-filter format and modality tokens to file extensions).
/// Resolved per request from [`ServiceState`](crate::service::ServiceState).
#[derive(Clone)]
pub struct WorkspaceDocumentService {
    postgres: PgClient,
    engine: EngineService,
}

impl WorkspaceDocumentService {
    /// Creates a [`WorkspaceDocumentService`] over its clients.
    #[must_use]
    pub fn new(postgres: PgClient, engine: EngineService) -> Self {
        Self { postgres, engine }
    }

    /// Lists a workspace's documents with cursor pagination, each with its backing
    /// blob and creator, resolving the query's format and modality facets against
    /// the engine's codec registry.
    pub async fn list(
        &self,
        workspace_id: Uuid,
        pagination: CursorPagination<DocumentCursor>,
        query: ListDocumentsInput,
    ) -> Result<CursorPage<WithAccountRef<DocumentWithBlob>>> {
        let filter = query.to_filter(&self.engine).map_err(|err| {
            ErrorKind::BadRequest
                .with_message("Unknown document format filter")
                .with_context(err.to_string())
        })?;

        let mut conn = self.postgres.get_connection().await?;
        Ok(conn
            .cursor_list_workspace_documents(workspace_id, pagination, filter)
            .await?)
    }

    /// Finds a document by id within a workspace, with its backing blob and
    /// creator, or a `NotFound`.
    pub async fn find(
        &self,
        workspace_id: Uuid,
        document_id: Uuid,
    ) -> Result<WithAccountRef<DocumentWithBlob>> {
        let mut conn = self.postgres.get_connection().await?;
        find_document_with_creator(&mut conn, workspace_id, document_id).await
    }

    /// Updates a document's metadata, returning it with its backing blob and
    /// creator. The update and its event commit together.
    pub async fn update(
        &self,
        origin: event::EventOrigin<'_>,
        document_id: Uuid,
        input: UpdateDocumentInput,
    ) -> Result<WithAccountRef<DocumentWithBlob>> {
        let mut conn = self.postgres.get_connection().await?;
        find_document(&mut conn, origin.workspace_id, document_id).await?;

        let document_updates = input.into_model();
        conn.transaction(async |conn| {
            let updated = conn
                .update_workspace_document(document_id, document_updates)
                .await?;
            conn.emit_event(
                origin,
                event::WorkspaceEvent::DocumentUpdated(event::DocumentUpdated {
                    document_id,
                    document_name: updated.display_name.clone(),
                }),
            )
            .await?;
            Ok::<(), Error>(())
        })
        .await?;

        tracing::info!(target: TRACING_TARGET, "Document updated");
        find_document_with_creator(&mut conn, origin.workspace_id, document_id).await
    }

    /// Soft-deletes a document, dropping its blob reference, and records the event
    /// atomically. The backing object is not removed here: the reaper reclaims a
    /// blob once its last reference is gone and its retention window has passed.
    pub async fn delete(&self, origin: event::EventOrigin<'_>, document_id: Uuid) -> Result<()> {
        let mut conn = self.postgres.get_connection().await?;
        let document = find_document(&mut conn, origin.workspace_id, document_id).await?;

        conn.transaction(async |conn| {
            conn.delete_workspace_document(document.id).await?;
            conn.emit_event(
                origin,
                event::WorkspaceEvent::DocumentDeleted(event::DocumentDeleted {
                    document_id,
                    document_name: document.display_name.clone(),
                }),
            )
            .await?;
            Ok::<(), Error>(())
        })
        .await?;

        tracing::info!(target: TRACING_TARGET, "Document deleted");
        Ok(())
    }

    /// Soft-deletes the live documents among the request's ids in one transaction,
    /// dropping each one's blob reference and recording a deletion event, and
    /// reports which ids were deleted versus skipped.
    ///
    /// Idempotent: an id that is unknown, already deleted, in another workspace, or
    /// held by an in-progress detection is reported as skipped rather than failing.
    pub async fn bulk_delete(
        &self,
        origin: event::EventOrigin<'_>,
        document_ids: Vec<Uuid>,
    ) -> Result<BulkDeleteOutcome> {
        let requested: BTreeSet<Uuid> = document_ids.into_iter().collect();
        let requested: Vec<Uuid> = requested.into_iter().collect();

        let mut conn = self.postgres.get_connection().await?;
        let documents = conn
            .transaction(async |conn| {
                let documents = conn
                    .delete_documents_in_workspace(origin.workspace_id, &requested)
                    .await?;
                for document in &documents {
                    conn.emit_event(
                        origin,
                        event::WorkspaceEvent::DocumentDeleted(event::DocumentDeleted {
                            document_id: document.id,
                            document_name: document.display_name.clone(),
                        }),
                    )
                    .await?;
                }
                Ok::<_, Error>(documents)
            })
            .await?;

        let deleted_ids: BTreeSet<Uuid> = documents.iter().map(|document| document.id).collect();
        let skipped: Vec<Uuid> = requested
            .into_iter()
            .filter(|id| !deleted_ids.contains(id))
            .collect();
        let deleted: Vec<Uuid> = deleted_ids.into_iter().collect();

        tracing::info!(
            target: TRACING_TARGET,
            deleted = deleted.len(),
            skipped = skipped.len(),
            "Documents bulk-deleted",
        );
        Ok(BulkDeleteOutcome { deleted, skipped })
    }
}

/// Finds a document within a workspace, or a `NotFound` error.
async fn find_document(
    conn: &mut PgConn,
    workspace_id: Uuid,
    document_id: Uuid,
) -> Result<DocumentModel> {
    conn.find_document_in_workspace(workspace_id, document_id)
        .await?
        .ok_or_else(|| Error::not_found("document"))
}

/// Finds a document within a workspace, with its backing blob and uploader's
/// identity, or a `NotFound` error.
async fn find_document_with_creator(
    conn: &mut PgConn,
    workspace_id: Uuid,
    document_id: Uuid,
) -> Result<WithAccountRef<DocumentWithBlob>> {
    conn.find_document_in_workspace_with_creator(workspace_id, document_id)
        .await?
        .ok_or_else(|| Error::not_found("document"))
}
