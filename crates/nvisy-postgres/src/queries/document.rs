//! Document repository for managing core document table operations.

use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use time::OffsetDateTime;
use uuid::Uuid;

use super::Pagination;
use crate::models::{Document, NewDocument, UpdateDocument};
use crate::types::DocumentStatus;
use crate::{PgError, PgResult, schema};

/// Repository for document table operations.
#[derive(Debug, Default, Clone, Copy)]
pub struct DocumentRepository;

impl DocumentRepository {
    /// Creates a new document repository instance.
    pub fn new() -> Self {
        Self
    }

    /// Creates a new document in the database.
    pub async fn create_document(
        conn: &mut AsyncPgConnection,
        new_document: NewDocument,
    ) -> PgResult<Document> {
        use schema::documents;

        diesel::insert_into(documents::table)
            .values(&new_document)
            .returning(Document::as_returning())
            .get_result(conn)
            .await
            .map_err(PgError::from)
    }

    /// Finds a document by its ID.
    pub async fn find_document_by_id(
        conn: &mut AsyncPgConnection,
        document_id: Uuid,
    ) -> PgResult<Option<Document>> {
        use schema::documents::{self, dsl};

        documents::table
            .filter(dsl::id.eq(document_id))
            .filter(dsl::deleted_at.is_null())
            .select(Document::as_select())
            .first(conn)
            .await
            .optional()
            .map_err(PgError::from)
    }

    /// Finds documents by project ID.
    pub async fn find_documents_by_project(
        conn: &mut AsyncPgConnection,
        project_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<Document>> {
        use schema::documents::{self, dsl};

        documents::table
            .filter(dsl::project_id.eq(project_id))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::updated_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(Document::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)
    }

    /// Finds documents by account ID.
    pub async fn find_documents_by_account(
        conn: &mut AsyncPgConnection,
        account_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<Document>> {
        use schema::documents::{self, dsl};

        documents::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::updated_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(Document::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)
    }

    /// Updates a document by ID.
    pub async fn update_document(
        conn: &mut AsyncPgConnection,
        document_id: Uuid,
        updates: UpdateDocument,
    ) -> PgResult<Document> {
        use schema::documents::{self, dsl};

        diesel::update(documents::table.filter(dsl::id.eq(document_id)))
            .set(&updates)
            .returning(Document::as_returning())
            .get_result(conn)
            .await
            .map_err(PgError::from)
    }

    /// Soft deletes a document by setting deleted_at timestamp.
    pub async fn delete_document(conn: &mut AsyncPgConnection, document_id: Uuid) -> PgResult<()> {
        use schema::documents::{self, dsl};

        diesel::update(documents::table.filter(dsl::id.eq(document_id)))
            .set(dsl::deleted_at.eq(Some(OffsetDateTime::now_utc())))
            .execute(conn)
            .await
            .map_err(PgError::from)?;

        Ok(())
    }

    /// Lists documents with pagination and optional filtering.
    pub async fn list_documents(
        conn: &mut AsyncPgConnection,
        pagination: Pagination,
    ) -> PgResult<Vec<Document>> {
        use schema::documents::{self, dsl};

        documents::table
            .filter(dsl::deleted_at.is_null())
            .order(dsl::updated_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(Document::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)
    }

    /// Searches documents by name or description.
    pub async fn search_documents(
        conn: &mut AsyncPgConnection,
        search_query: &str,
        project_id: Option<Uuid>,
        pagination: Pagination,
    ) -> PgResult<Vec<Document>> {
        use schema::documents::{self, dsl};

        let search_pattern = format!("%{}%", search_query.to_lowercase());

        let mut query = documents::table
            .filter(dsl::deleted_at.is_null())
            .filter(
                dsl::display_name
                    .ilike(&search_pattern)
                    .or(dsl::description.ilike(&search_pattern)),
            )
            .order(dsl::display_name.asc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(Document::as_select())
            .into_boxed();

        if let Some(proj_id) = project_id {
            query = query.filter(dsl::project_id.eq(proj_id));
        }

        query.load(conn).await.map_err(PgError::from)
    }

    /// Finds documents by status.
    pub async fn find_documents_by_status(
        conn: &mut AsyncPgConnection,
        status: DocumentStatus,
        pagination: Pagination,
    ) -> PgResult<Vec<Document>> {
        use schema::documents::{self, dsl};

        documents::table
            .filter(dsl::status.eq(status))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::updated_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(Document::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)
    }

    /// Finds template documents.
    pub async fn find_template_documents(
        conn: &mut AsyncPgConnection,
        pagination: Pagination,
    ) -> PgResult<Vec<Document>> {
        use schema::documents::{self, dsl};

        documents::table
            .filter(dsl::is_template.eq(true))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::display_name.asc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(Document::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)
    }

    // NOTE: Commented out because template_id field no longer exists in schema
    // /// Finds documents created from a specific template.
    // pub async fn find_documents_by_template(
    //     conn: &mut AsyncPgConnection,
    //     template_id: Uuid,
    //     pagination: Pagination,
    // ) -> PgResult<Vec<Document>> {
    //     use schema::documents::{self, dsl};
    //
    //     documents::table
    //         .filter(dsl::template_id.eq(template_id))
    //         .filter(dsl::deleted_at.is_null())
    //         .order(dsl::created_at.desc())
    //         .limit(pagination.limit)
    //         .offset(pagination.offset)
    //         .select(Document::as_select())
    //         .load(conn)
    //         .await
    //         .map_err(PgError::from)
    // }

    /// Finds recently created documents.
    pub async fn find_recently_created_documents(
        conn: &mut AsyncPgConnection,
        pagination: Pagination,
    ) -> PgResult<Vec<Document>> {
        use schema::documents::{self, dsl};

        let seven_days_ago = OffsetDateTime::now_utc() - time::Duration::days(7);

        documents::table
            .filter(dsl::created_at.gt(seven_days_ago))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(Document::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)
    }

    /// Finds recently updated documents.
    pub async fn find_recently_updated_documents(
        conn: &mut AsyncPgConnection,
        pagination: Pagination,
    ) -> PgResult<Vec<Document>> {
        use schema::documents::{self, dsl};

        let seven_days_ago = OffsetDateTime::now_utc() - time::Duration::days(7);

        documents::table
            .filter(dsl::updated_at.gt(seven_days_ago))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::updated_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(Document::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)
    }

    // NOTE: Commented out because archived_at field no longer exists in schema
    // /// Archives a document.
    // pub async fn archive_document(
    //     conn: &mut AsyncPgConnection,
    //     document_id: Uuid,
    // ) -> PgResult<Document> {
    //     Self::update_document(
    //         conn,
    //         document_id,
    //         UpdateDocument {
    //             archived_at: Some(OffsetDateTime::now_utc()),
    //             ..Default::default()
    //         },
    //     )
    //     .await
    // }
    //
    // /// Unarchives a document.
    // pub async fn unarchive_document(
    //     conn: &mut AsyncPgConnection,
    //     document_id: Uuid,
    // ) -> PgResult<Document> {
    //     Self::update_document(
    //         conn,
    //         document_id,
    //         UpdateDocument {
    //             archived_at: None,
    //             ..Default::default()
    //         },
    //     )
    //     .await
    // }

    // Statistics and maintenance

    /// Gets document statistics for a project.
    pub async fn get_project_document_stats(
        conn: &mut AsyncPgConnection,
        project_id: Uuid,
    ) -> PgResult<DocumentProjectStats> {
        use schema::documents::{self, dsl};

        let now = OffsetDateTime::now_utc();
        let seven_days_ago = now - time::Duration::days(7);

        // Total documents
        let total_count: i64 = documents::table
            .filter(dsl::project_id.eq(project_id))
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        // NOTE: archived_at field no longer exists, setting to 0
        let archived_count: i64 = 0;

        // Recently created documents
        let recent_count: i64 = documents::table
            .filter(dsl::project_id.eq(project_id))
            .filter(dsl::created_at.gt(seven_days_ago))
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        // Template documents
        let template_count: i64 = documents::table
            .filter(dsl::project_id.eq(project_id))
            .filter(dsl::is_template.eq(true))
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        Ok(DocumentProjectStats {
            total_count,
            archived_count,
            recent_count,
            template_count,
        })
    }

    /// Gets document statistics for a user.
    pub async fn get_user_document_stats(
        conn: &mut AsyncPgConnection,
        account_id: Uuid,
    ) -> PgResult<DocumentUserStats> {
        use schema::documents::{self, dsl};

        let now = OffsetDateTime::now_utc();
        let thirty_days_ago = now - time::Duration::days(30);

        // Total documents created by user
        let total_count: i64 = documents::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        // Recently created documents
        let recent_count: i64 = documents::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::created_at.gt(thirty_days_ago))
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        // Templates created by user
        let template_count: i64 = documents::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::is_template.eq(true))
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        Ok(DocumentUserStats {
            total_count,
            recent_count,
            template_count,
        })
    }

    /// Checks if a user has access to a document.
    pub async fn check_document_access(
        conn: &mut AsyncPgConnection,
        document_id: Uuid,
        account_id: Uuid,
    ) -> PgResult<bool> {
        use schema::documents::{self, dsl};

        let count: i64 = documents::table
            .filter(dsl::id.eq(document_id))
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        Ok(count > 0)
    }
}

/// Statistics for documents within a project.
#[derive(Debug, Clone, PartialEq)]
pub struct DocumentProjectStats {
    /// Total number of documents in the project
    pub total_count: i64,
    /// Number of archived documents
    pub archived_count: i64,
    /// Number of documents created in last 7 days
    pub recent_count: i64,
    /// Number of template documents
    pub template_count: i64,
}

impl DocumentProjectStats {
    /// Returns the archive rate as a percentage (0-100).
    pub fn archive_rate(&self) -> f64 {
        if self.total_count == 0 {
            0.0
        } else {
            (self.archived_count as f64 / self.total_count as f64) * 100.0
        }
    }

    /// Returns the template ratio as a percentage (0-100).
    pub fn template_ratio(&self) -> f64 {
        if self.total_count == 0 {
            0.0
        } else {
            (self.template_count as f64 / self.total_count as f64) * 100.0
        }
    }

    /// Returns whether the project has recent activity.
    pub fn has_recent_activity(&self) -> bool {
        self.recent_count > 0
    }
}

/// Statistics for documents created by a user.
#[derive(Debug, Clone, PartialEq)]
pub struct DocumentUserStats {
    /// Total number of documents created by user
    pub total_count: i64,
    /// Number of documents created in last 30 days
    pub recent_count: i64,
    /// Number of templates created by user
    pub template_count: i64,
}

impl DocumentUserStats {
    /// Returns whether the user is actively creating documents.
    pub fn is_active_creator(&self) -> bool {
        self.recent_count > 0
    }

    /// Returns whether the user creates templates.
    pub fn creates_templates(&self) -> bool {
        self.template_count > 0
    }
}
