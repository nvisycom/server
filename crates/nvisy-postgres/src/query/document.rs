//! Document repository for managing comprehensive document operations.

use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use time::OffsetDateTime;
use uuid::Uuid;

use super::Pagination;
use crate::model::{Document, NewDocument, UpdateDocument};
use crate::types::DocumentStatus;
use crate::{PgError, PgResult, schema};

/// Repository for comprehensive document database operations.
///
/// Provides database operations for managing documents throughout their lifecycle,
/// including creation, updates, status management, search functionality, and
/// analytics. This repository handles all database interactions related to
/// document management and serves as the primary interface for document data
/// within projects and collaborative workflows.
///
/// The repository supports document visibility controls, status tracking,
/// project-based organization, and comprehensive search and filtering capabilities
/// to enable rich document management and collaboration experiences. Documents
/// are the core content entities that enable knowledge sharing, version control,
/// and collaborative editing within project workspaces.
#[derive(Debug, Default, Clone, Copy)]
pub struct DocumentRepository;

impl DocumentRepository {
    /// Creates a new document repository instance.
    ///
    /// Returns a new repository instance ready for database operations.
    /// Since the repository is stateless, this is equivalent to using
    /// `Default::default()` or accessing repository methods statically.
    ///
    /// # Returns
    ///
    /// A new `DocumentRepository` instance.
    pub fn new() -> Self {
        Self
    }

    /// Creates a new document in the database with complete initial setup.
    ///
    /// Initializes a new document within a project workspace with the provided
    /// metadata and content structure. The document is immediately available for
    /// collaboration, editing, and can be discovered through search interfaces.
    /// This is the primary method for document creation and content onboarding
    /// within project environments.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `new_document` - Complete document data including name, description, and project association
    ///
    /// # Returns
    ///
    /// The created `Document` with database-generated ID and timestamps,
    /// or a database error if the operation fails.
    ///
    /// # Business Impact
    ///
    /// - Document becomes immediately available for team collaboration
    /// - Creator automatically becomes document owner with full permissions
    /// - Document appears in project document listings and search results
    /// - Enables content creation and knowledge sharing workflows
    /// - Supports version control and collaborative editing capabilities
    pub async fn create_document(
        conn: &mut AsyncPgConnection,
        new_document: NewDocument,
    ) -> PgResult<Document> {
        use schema::documents;

        let document = diesel::insert_into(documents::table)
            .values(&new_document)
            .returning(Document::as_returning())
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        Ok(document)
    }

    /// Finds a document by its unique identifier.
    ///
    /// Retrieves a specific document using its UUID, automatically excluding
    /// soft-deleted documents. This is the primary method for accessing
    /// individual documents when you know the exact document ID, commonly
    /// used for document viewing, editing, and permission validation.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `document_id` - UUID of the document to retrieve
    ///
    /// # Returns
    ///
    /// The matching `Document` if found and not deleted, `None` if not found,
    /// or a database error if the query fails.
    pub async fn find_document_by_id(
        conn: &mut AsyncPgConnection,
        document_id: Uuid,
    ) -> PgResult<Option<Document>> {
        use schema::documents::{self, dsl};

        let document = documents::table
            .filter(dsl::id.eq(document_id))
            .filter(dsl::deleted_at.is_null())
            .select(Document::as_select())
            .first(conn)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(document)
    }

    /// Finds documents associated with a specific project.
    ///
    /// Retrieves a paginated list of documents within a project workspace,
    /// ordered by most recently updated first. This provides teams with
    /// current document activity and enables project-scoped document
    /// management and discovery workflows.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `project_id` - UUID of the project whose documents to retrieve
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of `Document` entries within the project, ordered by
    /// update time (most recent first), or a database error if the query fails.
    pub async fn find_documents_by_project(
        conn: &mut AsyncPgConnection,
        project_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<Document>> {
        use schema::documents::{self, dsl};

        let documents = documents::table
            .filter(dsl::project_id.eq(project_id))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::updated_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(Document::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)?;

        Ok(documents)
    }

    /// Finds documents created by a specific account.
    ///
    /// Retrieves a paginated list of documents where the specified account is
    /// the original creator. Results are ordered by most recently updated first,
    /// providing users with their document creation history and enabling
    /// personal document management workflows.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `account_id` - UUID of the account whose created documents to retrieve
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of `Document` entries created by the account, ordered by
    /// update time (most recent first), or a database error if the query fails.
    pub async fn find_documents_by_account(
        conn: &mut AsyncPgConnection,
        account_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<Document>> {
        use schema::documents::{self, dsl};

        let documents = documents::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::updated_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(Document::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)?;

        Ok(documents)
    }

    /// Updates a document with new information and metadata.
    ///
    /// Applies partial updates to an existing document using the provided
    /// update structure. Only fields set to `Some(value)` will be modified,
    /// while `None` fields remain unchanged. The updated_at timestamp is
    /// automatically updated to reflect the modification time.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `document_id` - UUID of the document to update
    /// * `updates` - Partial update data containing only fields to modify
    ///
    /// # Returns
    ///
    /// The updated `Document` with new values and timestamp,
    /// or a database error if the operation fails.
    pub async fn update_document(
        conn: &mut AsyncPgConnection,
        document_id: Uuid,
        updates: UpdateDocument,
    ) -> PgResult<Document> {
        use schema::documents::{self, dsl};

        let document = diesel::update(documents::table.filter(dsl::id.eq(document_id)))
            .set(&updates)
            .returning(Document::as_returning())
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        Ok(document)
    }

    /// Soft deletes a document by setting the deletion timestamp.
    ///
    /// Marks a document as deleted without permanently removing it from the
    /// database. This preserves content for audit purposes and compliance
    /// requirements while preventing the document from appearing in normal
    /// queries and user interfaces. Related content such as comments and
    /// versions are typically preserved for data integrity.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `document_id` - UUID of the document to soft delete
    ///
    /// # Returns
    ///
    /// `()` on successful deletion, or a database error if the operation fails.
    ///
    /// # Business Impact
    ///
    /// - Document immediately becomes inaccessible to users
    /// - All document content and metadata is preserved for audit purposes
    /// - Related entities (comments, versions, files) may need separate cleanup
    /// - Document no longer appears in search or project document listings
    ///
    /// # Important Considerations
    ///
    /// Consider the impact on collaborative workflows and dependent content
    /// before performing this operation. Implement proper cleanup procedures
    /// for associated files and versions.
    pub async fn delete_document(conn: &mut AsyncPgConnection, document_id: Uuid) -> PgResult<()> {
        use schema::documents::{self, dsl};

        diesel::update(documents::table.filter(dsl::id.eq(document_id)))
            .set(dsl::deleted_at.eq(Some(OffsetDateTime::now_utc())))
            .execute(conn)
            .await
            .map_err(PgError::from)?;

        Ok(())
    }

    /// Lists documents with pagination support across the entire system.
    ///
    /// Retrieves a paginated list of all active documents in the system,
    /// ordered by most recently updated first. This provides system-wide
    /// document visibility for administrative purposes and global document
    /// discovery interfaces.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of `Document` entries ordered by update time (most recent first),
    /// or a database error if the query fails.
    pub async fn list_documents(
        conn: &mut AsyncPgConnection,
        pagination: Pagination,
    ) -> PgResult<Vec<Document>> {
        use schema::documents::{self, dsl};

        let documents = documents::table
            .filter(dsl::deleted_at.is_null())
            .order(dsl::updated_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(Document::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)?;

        Ok(documents)
    }

    /// Searches documents by name or description with optional project filtering.
    ///
    /// Performs fuzzy text search across document names and descriptions,
    /// enabling users to discover content through natural language queries.
    /// Results can be optionally filtered to a specific project for scoped
    /// search experiences. Search is case-insensitive and supports partial
    /// matching to maximize content discoverability.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `search_query` - Search terms to match against document names and descriptions
    /// * `project_id` - Optional project UUID to limit search scope
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of matching `Document` entries ordered alphabetically by name,
    /// or a database error if the query fails.
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

        let documents = query.load(conn).await.map_err(PgError::from)?;
        Ok(documents)
    }

    /// Finds documents filtered by their current status.
    ///
    /// Retrieves documents based on their workflow status, enabling status-based
    /// document management and workflow coordination. This supports document
    /// lifecycle management by allowing teams to focus on documents at specific
    /// stages of the content creation and review process.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `status` - Document status to filter by
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of `Document` entries with the specified status, ordered by
    /// update time (most recent first), or a database error if the query fails.
    pub async fn find_documents_by_status(
        conn: &mut AsyncPgConnection,
        status: DocumentStatus,
        pagination: Pagination,
    ) -> PgResult<Vec<Document>> {
        use schema::documents::{self, dsl};

        let documents = documents::table
            .filter(dsl::status.eq(status))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::updated_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(Document::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)?;

        Ok(documents)
    }

    /// Finds recently created documents across the system.
    ///
    /// Retrieves documents created within the last seven days, providing
    /// visibility into recent content creation activity. This supports
    /// content discovery, team activity monitoring, and helps users
    /// stay current with new content in their collaborative environment.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of recently created `Document` entries ordered by creation time
    /// (most recent first), or a database error if the query fails.
    pub async fn find_recently_created_documents(
        conn: &mut AsyncPgConnection,
        pagination: Pagination,
    ) -> PgResult<Vec<Document>> {
        use schema::documents::{self, dsl};

        let seven_days_ago = OffsetDateTime::now_utc() - time::Duration::days(7);

        let documents = documents::table
            .filter(dsl::created_at.gt(seven_days_ago))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(Document::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)?;

        Ok(documents)
    }

    /// Finds recently updated documents across the system.
    ///
    /// Retrieves documents updated within the last seven days, providing
    /// visibility into recent content modification activity. This supports
    /// content discovery, collaboration tracking, and helps users stay
    /// current with evolving content in their workspace.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of recently updated `Document` entries ordered by update time
    /// (most recent first), or a database error if the query fails.
    pub async fn find_recently_updated_documents(
        conn: &mut AsyncPgConnection,
        pagination: Pagination,
    ) -> PgResult<Vec<Document>> {
        use schema::documents::{self, dsl};

        let seven_days_ago = OffsetDateTime::now_utc() - time::Duration::days(7);

        let documents = documents::table
            .filter(dsl::updated_at.gt(seven_days_ago))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::updated_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(Document::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)?;

        Ok(documents)
    }

    /// Checks if a user has access rights to a specific document.
    ///
    /// Validates whether a user account has permission to access a document
    /// based on ownership. This is a basic access control mechanism that
    /// verifies the user created the document. More sophisticated access
    /// control may need to consider project membership and shared permissions.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `document_id` - UUID of the document to check access for
    /// * `account_id` - UUID of the user account requesting access
    ///
    /// # Returns
    ///
    /// `true` if the user has access to the document, `false` otherwise,
    /// or a database error if the query fails.
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
