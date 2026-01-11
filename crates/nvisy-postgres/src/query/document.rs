//! Document repository for managing document operations.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use pgtrgm::expression_methods::TrgmExpressionMethods;
use uuid::Uuid;

use crate::model::{Document, NewDocument, UpdateDocument};
use crate::types::{CursorPage, CursorPagination, OffsetPagination};
use crate::{PgConnection, PgError, PgResult, schema};

/// Repository for document database operations.
///
/// Handles document lifecycle management including creation, updates,
/// and search functionality.
pub trait DocumentRepository {
    /// Creates a new document with the provided metadata.
    fn create_document(
        &mut self,
        new_document: NewDocument,
    ) -> impl Future<Output = PgResult<Document>> + Send;

    /// Finds a document by its unique identifier.
    fn find_document_by_id(
        &mut self,
        document_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<Document>>> + Send;

    /// Lists documents associated with a specific workspace with offset pagination.
    fn offset_list_workspace_documents(
        &mut self,
        workspace_id: Uuid,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<Document>>> + Send;

    /// Lists documents associated with a specific workspace with cursor pagination.
    fn cursor_list_workspace_documents(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination,
    ) -> impl Future<Output = PgResult<CursorPage<Document>>> + Send;

    /// Lists documents created by a specific account with offset pagination.
    fn offset_list_account_documents(
        &mut self,
        account_id: Uuid,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<Document>>> + Send;

    /// Lists documents created by a specific account with cursor pagination.
    fn cursor_list_account_documents(
        &mut self,
        account_id: Uuid,
        pagination: CursorPagination,
    ) -> impl Future<Output = PgResult<CursorPage<Document>>> + Send;

    /// Updates a document with new information and metadata.
    fn update_document(
        &mut self,
        document_id: Uuid,
        updates: UpdateDocument,
    ) -> impl Future<Output = PgResult<Document>> + Send;

    /// Soft deletes a document by setting the deletion timestamp.
    fn delete_document(&mut self, document_id: Uuid) -> impl Future<Output = PgResult<()>> + Send;

    /// Lists all documents with offset pagination.
    fn offset_list_documents(
        &mut self,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<Document>>> + Send;

    /// Searches documents by name or description with optional workspace filtering.
    fn search_documents(
        &mut self,
        search_query: &str,
        workspace_id: Option<Uuid>,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<Document>>> + Send;
}

impl DocumentRepository for PgConnection {
    async fn create_document(&mut self, new_document: NewDocument) -> PgResult<Document> {
        use schema::documents;

        let document = diesel::insert_into(documents::table)
            .values(&new_document)
            .returning(Document::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(document)
    }

    async fn find_document_by_id(&mut self, document_id: Uuid) -> PgResult<Option<Document>> {
        use schema::documents::{self, dsl};

        let document = documents::table
            .filter(dsl::id.eq(document_id))
            .filter(dsl::deleted_at.is_null())
            .select(Document::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(document)
    }

    async fn offset_list_workspace_documents(
        &mut self,
        workspace_id: Uuid,
        pagination: OffsetPagination,
    ) -> PgResult<Vec<Document>> {
        use schema::documents::{self, dsl};

        let documents = documents::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::updated_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(Document::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(documents)
    }

    async fn cursor_list_workspace_documents(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination,
    ) -> PgResult<CursorPage<Document>> {
        use diesel::dsl::count_star;
        use schema::documents::{self, dsl};

        let base_filter = dsl::workspace_id
            .eq(workspace_id)
            .and(dsl::deleted_at.is_null());

        let total = if pagination.include_count {
            Some(
                documents::table
                    .filter(base_filter)
                    .select(count_star())
                    .get_result(self)
                    .await
                    .map_err(PgError::from)?,
            )
        } else {
            None
        };

        let items = if let Some(cursor) = &pagination.after {
            let cursor_ts = jiff_diesel::Timestamp::from(cursor.timestamp);
            documents::table
                .filter(base_filter)
                .filter(
                    dsl::updated_at
                        .lt(cursor_ts)
                        .or(dsl::updated_at.eq(cursor_ts).and(dsl::id.lt(cursor.id))),
                )
                .order((dsl::updated_at.desc(), dsl::id.desc()))
                .limit(pagination.fetch_limit())
                .select(Document::as_select())
                .load(self)
                .await
                .map_err(PgError::from)?
        } else {
            documents::table
                .filter(base_filter)
                .order((dsl::updated_at.desc(), dsl::id.desc()))
                .limit(pagination.fetch_limit())
                .select(Document::as_select())
                .load(self)
                .await
                .map_err(PgError::from)?
        };

        Ok(CursorPage::new(items, total, pagination.limit, |d| {
            (d.updated_at.into(), d.id)
        }))
    }

    async fn offset_list_account_documents(
        &mut self,
        account_id: Uuid,
        pagination: OffsetPagination,
    ) -> PgResult<Vec<Document>> {
        use schema::documents::{self, dsl};

        let documents = documents::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::updated_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(Document::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(documents)
    }

    async fn cursor_list_account_documents(
        &mut self,
        account_id: Uuid,
        pagination: CursorPagination,
    ) -> PgResult<CursorPage<Document>> {
        use diesel::dsl::count_star;
        use schema::documents::{self, dsl};

        let base_filter = dsl::account_id
            .eq(account_id)
            .and(dsl::deleted_at.is_null());

        let total = if pagination.include_count {
            Some(
                documents::table
                    .filter(base_filter)
                    .select(count_star())
                    .get_result(self)
                    .await
                    .map_err(PgError::from)?,
            )
        } else {
            None
        };

        let items = if let Some(cursor) = &pagination.after {
            let cursor_ts = jiff_diesel::Timestamp::from(cursor.timestamp);
            documents::table
                .filter(base_filter)
                .filter(
                    dsl::updated_at
                        .lt(cursor_ts)
                        .or(dsl::updated_at.eq(cursor_ts).and(dsl::id.lt(cursor.id))),
                )
                .order((dsl::updated_at.desc(), dsl::id.desc()))
                .limit(pagination.fetch_limit())
                .select(Document::as_select())
                .load(self)
                .await
                .map_err(PgError::from)?
        } else {
            documents::table
                .filter(base_filter)
                .order((dsl::updated_at.desc(), dsl::id.desc()))
                .limit(pagination.fetch_limit())
                .select(Document::as_select())
                .load(self)
                .await
                .map_err(PgError::from)?
        };

        Ok(CursorPage::new(items, total, pagination.limit, |d| {
            (d.updated_at.into(), d.id)
        }))
    }

    async fn update_document(
        &mut self,
        document_id: Uuid,
        updates: UpdateDocument,
    ) -> PgResult<Document> {
        use schema::documents::{self, dsl};

        let document = diesel::update(documents::table.filter(dsl::id.eq(document_id)))
            .set(&updates)
            .returning(Document::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(document)
    }

    async fn delete_document(&mut self, document_id: Uuid) -> PgResult<()> {
        use diesel::dsl::now;
        use schema::documents::{self, dsl};

        diesel::update(documents::table.filter(dsl::id.eq(document_id)))
            .set(dsl::deleted_at.eq(now))
            .execute(self)
            .await
            .map_err(PgError::from)?;

        Ok(())
    }

    async fn offset_list_documents(
        &mut self,
        pagination: OffsetPagination,
    ) -> PgResult<Vec<Document>> {
        use schema::documents::{self, dsl};

        let documents = documents::table
            .filter(dsl::deleted_at.is_null())
            .order(dsl::updated_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(Document::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(documents)
    }

    async fn search_documents(
        &mut self,
        search_query: &str,
        workspace_id: Option<Uuid>,
        pagination: OffsetPagination,
    ) -> PgResult<Vec<Document>> {
        use schema::documents::{self, dsl};

        let mut query = documents::table
            .filter(dsl::deleted_at.is_null())
            .filter(dsl::display_name.trgm_similar_to(search_query))
            .order(dsl::display_name.asc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(Document::as_select())
            .into_boxed();

        if let Some(ws_id) = workspace_id {
            query = query.filter(dsl::workspace_id.eq(ws_id));
        }

        let documents = query.load(self).await.map_err(PgError::from)?;
        Ok(documents)
    }
}
