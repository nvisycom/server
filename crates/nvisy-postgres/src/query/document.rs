//! Document repository for managing document operations.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use jiff::Timestamp;
use uuid::Uuid;

use super::Pagination;
use crate::model::{Document, NewDocument, UpdateDocument};
use crate::types::DocumentStatus;
use crate::{PgClient, PgError, PgResult, schema};

/// Repository for document database operations.
///
/// Handles document lifecycle management including creation, updates, status tracking,
/// and search functionality.
pub trait DocumentRepository {
    /// Creates a new document with the provided metadata.
    fn create_document(
        &self,
        new_document: NewDocument,
    ) -> impl Future<Output = PgResult<Document>> + Send;

    /// Finds a document by its unique identifier.
    fn find_document_by_id(
        &self,
        document_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<Document>>> + Send;

    /// Finds documents associated with a specific project.
    fn find_documents_by_project(
        &self,
        project_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<Document>>> + Send;

    /// Finds documents created by a specific account.
    fn find_documents_by_account(
        &self,
        account_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<Document>>> + Send;

    /// Updates a document with new information and metadata.
    fn update_document(
        &self,
        document_id: Uuid,
        updates: UpdateDocument,
    ) -> impl Future<Output = PgResult<Document>> + Send;

    /// Soft deletes a document by setting the deletion timestamp.
    fn delete_document(&self, document_id: Uuid) -> impl Future<Output = PgResult<()>> + Send;

    /// Lists documents with pagination support.
    fn list_documents(
        &self,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<Document>>> + Send;

    /// Searches documents by name or description with optional project filtering.
    fn search_documents(
        &self,
        search_query: &str,
        project_id: Option<Uuid>,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<Document>>> + Send;

    /// Finds documents filtered by their current status.
    fn find_documents_by_status(
        &self,
        status: DocumentStatus,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<Document>>> + Send;
}

impl DocumentRepository for PgClient {
    async fn create_document(&self, new_document: NewDocument) -> PgResult<Document> {
        let mut conn = self.get_connection().await?;

        use schema::documents;

        let document = diesel::insert_into(documents::table)
            .values(&new_document)
            .returning(Document::as_returning())
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(document)
    }

    async fn find_document_by_id(&self, document_id: Uuid) -> PgResult<Option<Document>> {
        let mut conn = self.get_connection().await?;

        use schema::documents::{self, dsl};

        let document = documents::table
            .filter(dsl::id.eq(document_id))
            .filter(dsl::deleted_at.is_null())
            .select(Document::as_select())
            .first(&mut conn)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(document)
    }

    async fn find_documents_by_project(
        &self,
        project_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<Document>> {
        let mut conn = self.get_connection().await?;

        use schema::documents::{self, dsl};

        let documents = documents::table
            .filter(dsl::project_id.eq(project_id))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::updated_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(Document::as_select())
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(documents)
    }

    async fn find_documents_by_account(
        &self,
        account_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<Document>> {
        let mut conn = self.get_connection().await?;

        use schema::documents::{self, dsl};

        let documents = documents::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::updated_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(Document::as_select())
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(documents)
    }

    async fn update_document(
        &self,
        document_id: Uuid,
        updates: UpdateDocument,
    ) -> PgResult<Document> {
        let mut conn = self.get_connection().await?;

        use schema::documents::{self, dsl};

        let document = diesel::update(documents::table.filter(dsl::id.eq(document_id)))
            .set(&updates)
            .returning(Document::as_returning())
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(document)
    }

    async fn delete_document(&self, document_id: Uuid) -> PgResult<()> {
        let mut conn = self.get_connection().await?;

        use schema::documents::{self, dsl};

        diesel::update(documents::table.filter(dsl::id.eq(document_id)))
            .set(dsl::deleted_at.eq(Some(jiff_diesel::Timestamp::from(Timestamp::now()))))
            .execute(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(())
    }

    async fn list_documents(&self, pagination: Pagination) -> PgResult<Vec<Document>> {
        let mut conn = self.get_connection().await?;

        use schema::documents::{self, dsl};

        let documents = documents::table
            .filter(dsl::deleted_at.is_null())
            .order(dsl::updated_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(Document::as_select())
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(documents)
    }

    async fn search_documents(
        &self,
        search_query: &str,
        project_id: Option<Uuid>,
        pagination: Pagination,
    ) -> PgResult<Vec<Document>> {
        let mut conn = self.get_connection().await?;

        use schema::documents::{self, dsl};

        let search_pattern = format!("%{}%", search_query.to_lowercase());

        let mut query = documents::table
            .filter(dsl::deleted_at.is_null())
            .filter(diesel::BoolExpressionMethods::or(
                dsl::display_name.ilike(&search_pattern),
                dsl::description.ilike(&search_pattern),
            ))
            .order(dsl::display_name.asc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(Document::as_select())
            .into_boxed();

        if let Some(proj_id) = project_id {
            query = query.filter(dsl::project_id.eq(proj_id));
        }

        let documents = query.load(&mut conn).await.map_err(PgError::from)?;
        Ok(documents)
    }

    async fn find_documents_by_status(
        &self,
        status: DocumentStatus,
        pagination: Pagination,
    ) -> PgResult<Vec<Document>> {
        let mut conn = self.get_connection().await?;

        use schema::documents::{self, dsl};

        let documents = documents::table
            .filter(dsl::status.eq(status))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::updated_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(Document::as_select())
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(documents)
    }
}
