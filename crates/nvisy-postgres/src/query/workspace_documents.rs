//! Workspace documents repository for managing human-facing files.
//!
//! A document is the human-facing file: an uploaded/imported original or a
//! redacted output. Its bytes live in a [`Blob`](crate::model::Blob), shared and
//! ref-counted; creating a document records a reference to its blob and
//! soft-deleting one drops that reference, both in the same transaction as the
//! document write. Retention and object reclamation live on the blob, so this
//! module has no expiry sweep — [`WorkspaceBlobRepository`] owns that.
//!
//! [`WorkspaceBlobRepository`]: crate::query::WorkspaceBlobRepository

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use jiff::Timestamp;
use pgtrgm::expression_methods::TrgmExpressionMethods;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::model::{
    Blob, NewBlob, NewWorkspaceDocument, NewWorkspaceDocumentExport, NewWorkspaceDocumentImport,
    UpdateWorkspaceDocument, WorkspaceDocument,
};
use crate::query::search::ilike_contains;
use crate::query::workspace_blobs::WorkspaceBlobRepository;
use crate::types::{
    AccountRefRow, CursorPage, CursorPagination, DocumentFilter, DocumentKind, WithAccountRef,
    keyset,
};
use crate::{Error, PgConnection, Result, schema};

/// Keyset for paginating a workspace's documents: newest first by `created_at`,
/// `id` as the tiebreaker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentCursor {
    /// When the document was created.
    pub created_at: Timestamp,
    /// Document id (tiebreaker).
    pub id: uuid::Uuid,
}

/// A document paired with its backing blob.
///
/// The document carries the human-facing fields (name, kind, creator); the blob
/// carries the content-addressed fields (extension, size, hash) and storage
/// location. Listings and lookups return the pair so a response never needs a
/// second query for the blob.
#[derive(Debug, Clone)]
pub struct DocumentWithBlob {
    /// The document.
    pub document: WorkspaceDocument,
    /// The document's backing blob.
    pub blob: Blob,
}

/// A live document imported from a connection, for deletion reconciliation.
///
/// A `source_key` no longer present in the remote listing identifies a document
/// whose source object was removed.
#[derive(Debug, Clone, Queryable)]
pub struct ImportedDocumentRef {
    /// The connection-side key the document was imported from.
    pub source_key: String,
    /// The imported document's id.
    pub document_id: Uuid,
    /// The backing blob's object-store path.
    pub storage_path: String,
}

/// Repository for workspace document database operations.
///
/// Handles document lifecycle management: upload/import tracking, blob
/// reference-counting on create and delete, export tracking, and listing.
pub trait WorkspaceDocumentRepository {
    /// Creates a document backed by `new_blob`, sharing an existing blob with
    /// identical content or inserting a fresh one, all in one transaction so the
    /// document and its blob reference commit together.
    fn create_workspace_document(
        &mut self,
        new_document: NewWorkspaceDocument,
        new_blob: NewBlob,
    ) -> impl Future<Output = Result<WorkspaceDocument>> + Send;

    /// Creates a document (backed by `new_blob`) and records its import origin
    /// (the connection and remote object key it came from) in one transaction.
    fn record_imported_document(
        &mut self,
        new_document: NewWorkspaceDocument,
        new_blob: NewBlob,
        connection_id: Uuid,
        source_key: String,
    ) -> impl Future<Output = Result<WorkspaceDocument>> + Send;

    /// Finds a document by its unique identifier.
    fn find_workspace_document_by_id(
        &mut self,
        document_id: Uuid,
    ) -> impl Future<Output = Result<Option<WorkspaceDocument>>> + Send;

    /// Finds a document by ID within a specific workspace.
    ///
    /// Provides workspace-scoped access control at the database level.
    fn find_document_in_workspace(
        &mut self,
        workspace_id: Uuid,
        document_id: Uuid,
    ) -> impl Future<Output = Result<Option<WorkspaceDocument>>> + Send;

    /// Finds a document by id within a workspace, paired with its backing blob and
    /// the handle and avatar of the account that created it.
    ///
    /// Provides workspace-scoped access control at the database level.
    fn find_document_in_workspace_with_creator(
        &mut self,
        workspace_id: Uuid,
        document_id: Uuid,
    ) -> impl Future<Output = Result<Option<WithAccountRef<DocumentWithBlob>>>> + Send;

    /// Returns the remote object keys already imported (live) from a connection.
    ///
    /// Used to skip re-importing objects during a connection sync.
    fn imported_keys_for_connection(
        &mut self,
        connection_id: Uuid,
    ) -> impl Future<Output = Result<Vec<String>>> + Send;

    /// Records that `document_id` was exported to `connection_id` under
    /// `remote_key`, so a scheduled redacted export does not push it again.
    fn record_exported_document(
        &mut self,
        document_id: Uuid,
        connection_id: Uuid,
        remote_key: String,
    ) -> impl Future<Output = Result<()>> + Send;

    /// Returns the live redacted documents in `connection`'s workspace that have
    /// not yet been exported to that connection. Backs scheduled export.
    fn redacted_documents_not_exported(
        &mut self,
        workspace_id: Uuid,
        connection_id: Uuid,
    ) -> impl Future<Output = Result<Vec<WorkspaceDocument>>> + Send;

    /// Returns each live document imported from a connection, for deletion
    /// reconciliation (a source key absent from the remote listing identifies a
    /// removed source object).
    fn imported_documents_for_connection(
        &mut self,
        connection_id: Uuid,
    ) -> impl Future<Output = Result<Vec<ImportedDocumentRef>>> + Send;

    /// Updates a document with new metadata or settings.
    fn update_workspace_document(
        &mut self,
        document_id: Uuid,
        updates: UpdateWorkspaceDocument,
    ) -> impl Future<Output = Result<WorkspaceDocument>> + Send;

    /// Soft-deletes a document and drops its blob reference, in one transaction.
    fn delete_workspace_document(
        &mut self,
        document_id: Uuid,
    ) -> impl Future<Output = Result<()>> + Send;

    /// Lists all documents in a workspace with cursor pagination and optional
    /// filtering, each paired with the handle and avatar of the account that
    /// created it.
    fn cursor_list_workspace_documents(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination<DocumentCursor>,
        filter: DocumentFilter,
    ) -> impl Future<Output = Result<CursorPage<WithAccountRef<DocumentWithBlob>>>> + Send;

    /// Soft-deletes the live documents among `document_ids` that belong to
    /// `workspace_id`, dropping each one's blob reference, and returns the rows it
    /// actually transitioned.
    ///
    /// Resolution and deletion are one atomic step: the `UPDATE ... RETURNING`
    /// guarded on `deleted_at IS NULL` transitions and returns only rows it
    /// changed, so a row concurrently deleted by another request is absent from
    /// the result and never double-reported. Ids that are unknown, already
    /// deleted, or in another workspace are simply absent rather than an error.
    /// Also drops each returned document's import-origin row so re-import is never
    /// blocked (see [`delete_workspace_document`]).
    ///
    /// [`delete_workspace_document`]: WorkspaceDocumentRepository::delete_workspace_document
    fn delete_documents_in_workspace(
        &mut self,
        workspace_id: Uuid,
        document_ids: &[Uuid],
    ) -> impl Future<Output = Result<Vec<WorkspaceDocument>>> + Send;
}

impl WorkspaceDocumentRepository for PgConnection {
    async fn create_workspace_document(
        &mut self,
        mut new_document: NewWorkspaceDocument,
        new_blob: NewBlob,
    ) -> Result<WorkspaceDocument> {
        use diesel_async::AsyncConnection;
        use schema::workspace_documents;

        self.transaction(async |conn| {
            let blob = conn.find_or_create_blob(new_blob).await?;
            new_document.blob_id = blob.id;

            let document = diesel::insert_into(workspace_documents::table)
                .values(&new_document)
                .returning(WorkspaceDocument::as_returning())
                .get_result(conn)
                .await
                .map_err(Error::from)?;

            Ok::<_, Error>(document)
        })
        .await
    }

    async fn record_imported_document(
        &mut self,
        mut new_document: NewWorkspaceDocument,
        new_blob: NewBlob,
        connection_id: Uuid,
        source_key: String,
    ) -> Result<WorkspaceDocument> {
        use diesel_async::AsyncConnection;
        use schema::{workspace_document_imports, workspace_documents};

        self.transaction(async |conn| {
            let blob = conn.find_or_create_blob(new_blob).await?;
            new_document.blob_id = blob.id;

            let document = diesel::insert_into(workspace_documents::table)
                .values(&new_document)
                .returning(WorkspaceDocument::as_returning())
                .get_result(conn)
                .await
                .map_err(Error::from)?;

            diesel::insert_into(workspace_document_imports::table)
                .values(NewWorkspaceDocumentImport {
                    document_id: document.id,
                    connection_id,
                    source_key,
                })
                .execute(conn)
                .await
                .map_err(Error::from)?;

            Ok::<_, Error>(document)
        })
        .await
    }

    async fn find_workspace_document_by_id(
        &mut self,
        document_id: Uuid,
    ) -> Result<Option<WorkspaceDocument>> {
        use schema::workspace_documents::{self, dsl};

        let document = workspace_documents::table
            .filter(dsl::id.eq(document_id))
            .filter(dsl::deleted_at.is_null())
            .select(WorkspaceDocument::as_select())
            .first(self)
            .await
            .optional()
            .map_err(Error::from)?;

        Ok(document)
    }

    async fn find_document_in_workspace(
        &mut self,
        workspace_id: Uuid,
        document_id: Uuid,
    ) -> Result<Option<WorkspaceDocument>> {
        use schema::workspace_documents::{self, dsl};

        let document = workspace_documents::table
            .filter(dsl::id.eq(document_id))
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .select(WorkspaceDocument::as_select())
            .first(self)
            .await
            .optional()
            .map_err(Error::from)?;

        Ok(document)
    }

    async fn find_document_in_workspace_with_creator(
        &mut self,
        workspace_id: Uuid,
        document_id: Uuid,
    ) -> Result<Option<WithAccountRef<DocumentWithBlob>>> {
        use schema::workspace_documents::dsl;
        use schema::{accounts, workspace_blobs, workspace_documents};

        let row = workspace_documents::table
            .inner_join(accounts::table)
            .inner_join(workspace_blobs::table)
            .filter(dsl::id.eq(document_id))
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .select((
                WorkspaceDocument::as_select(),
                Blob::as_select(),
                (
                    accounts::username,
                    accounts::display_name,
                    accounts::avatar_url,
                ),
            ))
            .first::<(WorkspaceDocument, Blob, AccountRefRow)>(self)
            .await
            .optional()
            .map_err(Error::from)?;

        Ok(row.map(|row| WithAccountRef {
            item: DocumentWithBlob {
                document: row.0,
                blob: row.1,
            },
            account: row.2,
        }))
    }

    async fn imported_keys_for_connection(&mut self, connection_id: Uuid) -> Result<Vec<String>> {
        use schema::{workspace_document_imports, workspace_documents};

        let keys = workspace_document_imports::table
            .inner_join(workspace_documents::table)
            .filter(workspace_document_imports::connection_id.eq(connection_id))
            .filter(workspace_documents::deleted_at.is_null())
            .select(workspace_document_imports::source_key)
            .load::<String>(self)
            .await
            .map_err(Error::from)?;

        Ok(keys)
    }

    async fn record_exported_document(
        &mut self,
        document_id: Uuid,
        connection_id: Uuid,
        remote_key: String,
    ) -> Result<()> {
        use schema::workspace_document_exports;

        diesel::insert_into(workspace_document_exports::table)
            .values(NewWorkspaceDocumentExport {
                document_id,
                connection_id,
                remote_key,
            })
            .on_conflict((
                workspace_document_exports::document_id,
                workspace_document_exports::connection_id,
            ))
            .do_update()
            .set((
                workspace_document_exports::remote_key.eq(diesel::upsert::excluded(
                    workspace_document_exports::remote_key,
                )),
                workspace_document_exports::exported_at.eq(diesel::dsl::now),
            ))
            .execute(self)
            .await
            .map_err(Error::from)?;

        Ok(())
    }

    async fn redacted_documents_not_exported(
        &mut self,
        workspace_id: Uuid,
        connection_id: Uuid,
    ) -> Result<Vec<WorkspaceDocument>> {
        use schema::workspace_document_exports;
        use schema::workspace_documents::{self, dsl};

        let exported = workspace_document_exports::table
            .filter(workspace_document_exports::connection_id.eq(connection_id))
            .select(workspace_document_exports::document_id);

        let documents = workspace_documents::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::kind.eq(DocumentKind::Redacted))
            .filter(dsl::deleted_at.is_null())
            .filter(dsl::id.ne_all(exported))
            .select(WorkspaceDocument::as_select())
            .load(self)
            .await
            .map_err(Error::from)?;

        Ok(documents)
    }

    async fn imported_documents_for_connection(
        &mut self,
        connection_id: Uuid,
    ) -> Result<Vec<ImportedDocumentRef>> {
        use schema::{workspace_blobs, workspace_document_imports, workspace_documents};

        let documents = workspace_document_imports::table
            .inner_join(workspace_documents::table.inner_join(workspace_blobs::table))
            .filter(workspace_document_imports::connection_id.eq(connection_id))
            .filter(workspace_documents::deleted_at.is_null())
            .select((
                workspace_document_imports::source_key,
                workspace_documents::id,
                workspace_blobs::storage_path,
            ))
            .load::<ImportedDocumentRef>(self)
            .await
            .map_err(Error::from)?;

        Ok(documents)
    }

    async fn update_workspace_document(
        &mut self,
        document_id: Uuid,
        updates: UpdateWorkspaceDocument,
    ) -> Result<WorkspaceDocument> {
        use schema::workspace_documents::{self, dsl};

        let document = diesel::update(workspace_documents::table.filter(dsl::id.eq(document_id)))
            .set(&updates)
            .returning(WorkspaceDocument::as_returning())
            .get_result(self)
            .await
            .map_err(Error::from)?;

        Ok(document)
    }

    async fn delete_workspace_document(&mut self, document_id: Uuid) -> Result<()> {
        use diesel_async::AsyncConnection;
        use schema::{workspace_document_imports, workspace_documents};

        // Soft-delete the document, drop its blob reference, and drop its
        // import-origin row (if any) atomically. The origin's `(connection_id,
        // source_key)` uniqueness would otherwise block ever re-importing that
        // source object, since the soft delete keeps the document row and its
        // `ON DELETE CASCADE` never fires.
        //
        // Guarded on `deleted_at IS NULL` so it is idempotent: the reaper's
        // reconcile sweep re-invokes this for an already-deleted row, and must not
        // move `deleted_at` forward or drop the blob reference twice.
        self.transaction(async |conn| {
            let deleted: Vec<Uuid> = diesel::update(
                workspace_documents::table
                    .filter(workspace_documents::id.eq(document_id))
                    .filter(workspace_documents::deleted_at.is_null()),
            )
            .set(workspace_documents::deleted_at.eq(diesel::dsl::now))
            .returning(workspace_documents::blob_id)
            .get_results(conn)
            .await
            .map_err(Error::from)?;

            // Only drop the blob reference if this call is the one that
            // transitioned the row (a second, concurrent delete returns nothing).
            for blob_id in &deleted {
                conn.decrement_ref(*blob_id).await?;
            }

            diesel::delete(
                workspace_document_imports::table
                    .filter(workspace_document_imports::document_id.eq(document_id)),
            )
            .execute(conn)
            .await
            .map_err(Error::from)?;

            Ok::<_, Error>(())
        })
        .await
    }

    async fn cursor_list_workspace_documents(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination<DocumentCursor>,
        filter: DocumentFilter,
    ) -> Result<CursorPage<WithAccountRef<DocumentWithBlob>>> {
        use schema::workspace_documents::dsl;
        use schema::{accounts, workspace_blobs, workspace_documents};

        // Precompute filter values
        let search_term = filter.search.clone();
        let extensions = filter.extensions.clone();
        let hash = filter.hash.clone();

        // The scoped builder (filters shared by the count and the page). Content
        // identity (extension, hash) lives on the blob, so the builder joins each
        // document to its blob; the join must apply to both the count and the page
        // or they disagree.
        let scoped = || {
            let mut query = workspace_documents::table
                .inner_join(accounts::table)
                .inner_join(workspace_blobs::table)
                .filter(dsl::workspace_id.eq(workspace_id))
                .filter(dsl::deleted_at.is_null())
                .into_boxed();

            // Hybrid name search: ILIKE substring (works for short queries) OR
            // trigram similarity (typo tolerance); both served by the trgm index.
            if let Some(ref term) = search_term {
                query = query.filter(
                    dsl::display_name
                        .ilike(ilike_contains(term))
                        .or(dsl::display_name.trgm_similar_to(term)),
                );
            }

            // Apply the extension constraint. A present-but-empty set matches
            // nothing (an active facet with no members), so apply whenever `Some`.
            if let Some(ref extensions) = extensions {
                query = query.filter(dsl::file_extension.eq_any(extensions));
            }

            // Apply the exact content-hash constraint (dedup lookup).
            if let Some(ref hash) = hash {
                query = query.filter(workspace_blobs::content_hash.eq(hash));
            }

            query
        };

        let total = if pagination.include_count {
            Some(
                scoped()
                    .count()
                    .get_result::<i64>(self)
                    .await
                    .map_err(Error::from)?,
            )
        } else {
            None
        };

        let after = pagination
            .after_key()
            .map(|k| (jiff_diesel::Timestamp::from(k.created_at), k.id));
        let rows: Vec<(WorkspaceDocument, Blob, AccountRefRow)> = keyset!(
            scoped(),
            dsl::created_at,
            dsl::id,
            pagination.direction,
            after
        )
        .select((
            WorkspaceDocument::as_select(),
            Blob::as_select(),
            (
                accounts::username,
                accounts::display_name,
                accounts::avatar_url,
            ),
        ))
        .limit(pagination.fetch_limit())
        .load(self)
        .await
        .map_err(Error::from)?;

        let items: Vec<WithAccountRef<DocumentWithBlob>> = rows
            .into_iter()
            .map(|row| WithAccountRef {
                item: DocumentWithBlob {
                    document: row.0,
                    blob: row.1,
                },
                account: row.2,
            })
            .collect();

        Ok(CursorPage::new(items, total, pagination.limit, |wc| {
            DocumentCursor {
                created_at: wc.item.document.created_at.into(),
                id: wc.item.document.id,
            }
        }))
    }

    async fn delete_documents_in_workspace(
        &mut self,
        workspace_id: Uuid,
        document_ids: &[Uuid],
    ) -> Result<Vec<WorkspaceDocument>> {
        use schema::{workspace_document_imports, workspace_documents};

        // Transition and return only the live rows in this workspace, in one
        // atomic statement. The `deleted_at IS NULL` guard means a row a concurrent
        // request already deleted is not returned here, so it is never
        // double-counted, double-emitted, or double-dereferenced. This runs on the
        // caller's connection (not its own transaction) so the caller can commit
        // the deletion together with the events it emits for the returned rows.
        let deleted: Vec<WorkspaceDocument> = diesel::update(
            workspace_documents::table
                .filter(workspace_documents::id.eq_any(document_ids))
                .filter(workspace_documents::workspace_id.eq(workspace_id))
                .filter(workspace_documents::deleted_at.is_null()),
        )
        .set(workspace_documents::deleted_at.eq(diesel::dsl::now))
        .returning(WorkspaceDocument::as_returning())
        .get_results(self)
        .await
        .map_err(Error::from)?;

        // Drop each deleted document's blob reference so a blob with no more live
        // references becomes reclaimable once its retention passes.
        let blob_ids: Vec<Uuid> = deleted.iter().map(|document| document.blob_id).collect();
        for blob_id in blob_ids {
            self.decrement_ref(blob_id).await?;
        }

        // Drop the import-origin rows of exactly the documents just deleted, so
        // re-import is never blocked (see `delete_workspace_document`).
        let deleted_ids: Vec<Uuid> = deleted.iter().map(|document| document.id).collect();
        diesel::delete(
            workspace_document_imports::table
                .filter(workspace_document_imports::document_id.eq_any(&deleted_ids)),
        )
        .execute(self)
        .await
        .map_err(Error::from)?;

        Ok(deleted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::{WorkspaceBlobRepository, WorkspaceDocumentRepository};
    use crate::test_util::TestDatabase;

    #[tokio::test]
    async fn create_find_update_and_soft_delete_drops_the_blob_reference() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        // Create a document backed by a fresh blob; the blob is referenced once.
        let document = conn
            .create_workspace_document(
                NewWorkspaceDocument::test(seeded.workspace_id, seeded.account_id, Uuid::nil()),
                NewBlob::test(seeded.workspace_id),
            )
            .await?;
        let blob = conn
            .find_blob_by_id(document.blob_id)
            .await?
            .expect("blob present");
        assert_eq!(blob.ref_count, 1);

        // Found by id within its workspace.
        assert!(
            conn.find_document_in_workspace(seeded.workspace_id, document.id)
                .await?
                .is_some()
        );

        // Update the display name.
        let updated = conn
            .update_workspace_document(
                document.id,
                UpdateWorkspaceDocument {
                    display_name: Some("renamed.txt".to_owned()),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(updated.display_name, "renamed.txt");

        // Soft-delete drops the document from lookups and drops the blob reference.
        conn.delete_workspace_document(document.id).await?;
        assert!(
            conn.find_document_in_workspace(seeded.workspace_id, document.id)
                .await?
                .is_none()
        );
        let after = conn
            .find_blob_by_id(document.blob_id)
            .await?
            .expect("blob still present");
        assert_eq!(after.ref_count, 0, "soft-delete drops the blob reference");

        // Idempotent: a repeat delete does not drop the reference again.
        conn.delete_workspace_document(document.id).await?;
        let floored = conn
            .find_blob_by_id(document.blob_id)
            .await?
            .expect("blob present");
        assert_eq!(floored.ref_count, 0);
        Ok(())
    }

    #[tokio::test]
    async fn two_documents_sharing_content_share_one_blob() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        // Two documents created from byte-identical blobs share one blob row, whose
        // reference count reaches two.
        let blob = NewBlob::test(seeded.workspace_id);
        let first = conn
            .create_workspace_document(
                NewWorkspaceDocument::test(seeded.workspace_id, seeded.account_id, Uuid::nil()),
                blob.clone(),
            )
            .await?;
        let second = conn
            .create_workspace_document(
                NewWorkspaceDocument::test(seeded.workspace_id, seeded.account_id, Uuid::nil()),
                blob,
            )
            .await?;
        assert_eq!(
            first.blob_id, second.blob_id,
            "identical content shares one blob"
        );
        assert_eq!(
            conn.find_blob_by_id(first.blob_id)
                .await?
                .expect("blob present")
                .ref_count,
            2
        );

        // Deleting one document leaves the shared blob live for the other.
        conn.delete_workspace_document(first.id).await?;
        assert_eq!(
            conn.find_blob_by_id(first.blob_id)
                .await?
                .expect("blob present")
                .ref_count,
            1,
            "the shared blob stays live while another document references it"
        );
        Ok(())
    }

    #[tokio::test]
    async fn bulk_delete_transitions_live_rows_and_reports_the_rest_skipped() -> anyhow::Result<()>
    {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        let one = conn
            .create_workspace_document(
                NewWorkspaceDocument::test(seeded.workspace_id, seeded.account_id, Uuid::nil()),
                NewBlob::test(seeded.workspace_id),
            )
            .await?;
        let two = conn
            .create_workspace_document(
                NewWorkspaceDocument::test(seeded.workspace_id, seeded.account_id, Uuid::nil()),
                NewBlob::test(seeded.workspace_id),
            )
            .await?;

        // One live id, one unknown id: only the live one is transitioned.
        let unknown = Uuid::now_v7();
        let deleted = conn
            .delete_documents_in_workspace(seeded.workspace_id, &[one.id, unknown])
            .await?;
        let deleted_ids: Vec<Uuid> = deleted.iter().map(|d| d.id).collect();
        assert_eq!(deleted_ids, vec![one.id]);

        // The deleted document's blob reference was dropped; the untouched one's
        // stays.
        assert_eq!(
            conn.find_blob_by_id(one.blob_id)
                .await?
                .expect("blob present")
                .ref_count,
            0
        );
        assert_eq!(
            conn.find_blob_by_id(two.blob_id)
                .await?
                .expect("blob present")
                .ref_count,
            1
        );
        Ok(())
    }
}
