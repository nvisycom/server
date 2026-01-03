//! Document chunks repository for managing document text segments and embeddings.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use pgvector::Vector;
use uuid::Uuid;

use crate::types::OffsetPagination;
use crate::model::{DocumentChunk, NewDocumentChunk, UpdateDocumentChunk};
use crate::{PgConnection, PgError, PgResult, schema};

/// Repository for document chunk database operations.
///
/// Handles chunk lifecycle management including creation, embedding updates,
/// and semantic similarity search via pgvector.
pub trait DocumentChunkRepository {
    /// Creates a new document chunk.
    fn create_document_chunk(
        &mut self,
        new_chunk: NewDocumentChunk,
    ) -> impl Future<Output = PgResult<DocumentChunk>> + Send;

    /// Creates multiple document chunks in a single transaction.
    fn create_document_chunks(
        &mut self,
        new_chunks: Vec<NewDocumentChunk>,
    ) -> impl Future<Output = PgResult<Vec<DocumentChunk>>> + Send;

    /// Finds a chunk by its unique identifier.
    fn find_document_chunk_by_id(
        &mut self,
        chunk_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<DocumentChunk>>> + Send;

    /// Lists all chunks for a specific file.
    fn list_file_chunks(
        &mut self,
        file_id: Uuid,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<DocumentChunk>>> + Send;

    /// Lists all chunks for a file ordered by chunk index.
    fn list_file_chunks_ordered(
        &mut self,
        file_id: Uuid,
    ) -> impl Future<Output = PgResult<Vec<DocumentChunk>>> + Send;

    /// Updates a chunk with new data.
    fn update_document_chunk(
        &mut self,
        chunk_id: Uuid,
        updates: UpdateDocumentChunk,
    ) -> impl Future<Output = PgResult<DocumentChunk>> + Send;

    /// Updates the embedding for a chunk.
    fn update_chunk_embedding(
        &mut self,
        chunk_id: Uuid,
        embedding: Vector,
        model: &str,
    ) -> impl Future<Output = PgResult<DocumentChunk>> + Send;

    /// Deletes a chunk by ID.
    fn delete_document_chunk(
        &mut self,
        chunk_id: Uuid,
    ) -> impl Future<Output = PgResult<()>> + Send;

    /// Deletes all chunks for a file.
    fn delete_file_chunks(&mut self, file_id: Uuid)
    -> impl Future<Output = PgResult<usize>> + Send;

    /// Finds chunks without embeddings.
    fn find_chunks_without_embeddings(
        &mut self,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<DocumentChunk>>> + Send;

    /// Finds chunks without embeddings for a specific file.
    fn find_file_chunks_without_embeddings(
        &mut self,
        file_id: Uuid,
    ) -> impl Future<Output = PgResult<Vec<DocumentChunk>>> + Send;

    /// Searches for similar chunks using cosine similarity.
    ///
    /// Returns chunks ordered by similarity (most similar first).
    fn search_similar_chunks(
        &mut self,
        query_embedding: Vector,
        limit: i64,
    ) -> impl Future<Output = PgResult<Vec<DocumentChunk>>> + Send;

    /// Searches for similar chunks within specific files.
    fn search_similar_chunks_in_files(
        &mut self,
        query_embedding: Vector,
        file_ids: &[Uuid],
        limit: i64,
    ) -> impl Future<Output = PgResult<Vec<DocumentChunk>>> + Send;

    /// Gets the total chunk count for a file.
    fn get_file_chunk_count(&mut self, file_id: Uuid)
    -> impl Future<Output = PgResult<i64>> + Send;
}

impl DocumentChunkRepository for PgConnection {
    async fn create_document_chunk(
        &mut self,
        new_chunk: NewDocumentChunk,
    ) -> PgResult<DocumentChunk> {
        use schema::document_chunks;

        let chunk = diesel::insert_into(document_chunks::table)
            .values(&new_chunk)
            .returning(DocumentChunk::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(chunk)
    }

    async fn create_document_chunks(
        &mut self,
        new_chunks: Vec<NewDocumentChunk>,
    ) -> PgResult<Vec<DocumentChunk>> {
        use schema::document_chunks;

        if new_chunks.is_empty() {
            return Ok(vec![]);
        }

        let chunks = diesel::insert_into(document_chunks::table)
            .values(&new_chunks)
            .returning(DocumentChunk::as_returning())
            .get_results(self)
            .await
            .map_err(PgError::from)?;

        Ok(chunks)
    }

    async fn find_document_chunk_by_id(
        &mut self,
        chunk_id: Uuid,
    ) -> PgResult<Option<DocumentChunk>> {
        use schema::document_chunks::{self, dsl};

        let chunk = document_chunks::table
            .filter(dsl::id.eq(chunk_id))
            .select(DocumentChunk::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(chunk)
    }

    async fn list_file_chunks(
        &mut self,
        file_id: Uuid,
        pagination: OffsetPagination,
    ) -> PgResult<Vec<DocumentChunk>> {
        use schema::document_chunks::{self, dsl};

        let chunks = document_chunks::table
            .filter(dsl::file_id.eq(file_id))
            .order(dsl::chunk_index.asc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentChunk::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(chunks)
    }

    async fn list_file_chunks_ordered(&mut self, file_id: Uuid) -> PgResult<Vec<DocumentChunk>> {
        use schema::document_chunks::{self, dsl};

        let chunks = document_chunks::table
            .filter(dsl::file_id.eq(file_id))
            .order(dsl::chunk_index.asc())
            .select(DocumentChunk::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(chunks)
    }

    async fn update_document_chunk(
        &mut self,
        chunk_id: Uuid,
        updates: UpdateDocumentChunk,
    ) -> PgResult<DocumentChunk> {
        use schema::document_chunks::{self, dsl};

        let chunk = diesel::update(document_chunks::table.filter(dsl::id.eq(chunk_id)))
            .set(&updates)
            .returning(DocumentChunk::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(chunk)
    }

    async fn update_chunk_embedding(
        &mut self,
        chunk_id: Uuid,
        embedding: Vector,
        model: &str,
    ) -> PgResult<DocumentChunk> {
        use schema::document_chunks::{self, dsl};

        let now = jiff_diesel::Timestamp::from(jiff::Timestamp::now());

        let chunk = diesel::update(document_chunks::table.filter(dsl::id.eq(chunk_id)))
            .set((
                dsl::embedding.eq(Some(embedding)),
                dsl::embedding_model.eq(Some(model)),
                dsl::embedded_at.eq(Some(now)),
            ))
            .returning(DocumentChunk::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(chunk)
    }

    async fn delete_document_chunk(&mut self, chunk_id: Uuid) -> PgResult<()> {
        use schema::document_chunks::{self, dsl};

        diesel::delete(document_chunks::table.filter(dsl::id.eq(chunk_id)))
            .execute(self)
            .await
            .map_err(PgError::from)?;

        Ok(())
    }

    async fn delete_file_chunks(&mut self, file_id: Uuid) -> PgResult<usize> {
        use schema::document_chunks::{self, dsl};

        let affected = diesel::delete(document_chunks::table.filter(dsl::file_id.eq(file_id)))
            .execute(self)
            .await
            .map_err(PgError::from)?;

        Ok(affected)
    }

    async fn find_chunks_without_embeddings(
        &mut self,
        pagination: OffsetPagination,
    ) -> PgResult<Vec<DocumentChunk>> {
        use schema::document_chunks::{self, dsl};

        let chunks = document_chunks::table
            .filter(dsl::embedding.is_null())
            .order(dsl::created_at.asc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentChunk::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(chunks)
    }

    async fn find_file_chunks_without_embeddings(
        &mut self,
        file_id: Uuid,
    ) -> PgResult<Vec<DocumentChunk>> {
        use schema::document_chunks::{self, dsl};

        let chunks = document_chunks::table
            .filter(dsl::file_id.eq(file_id))
            .filter(dsl::embedding.is_null())
            .order(dsl::chunk_index.asc())
            .select(DocumentChunk::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(chunks)
    }

    async fn search_similar_chunks(
        &mut self,
        query_embedding: Vector,
        limit: i64,
    ) -> PgResult<Vec<DocumentChunk>> {
        use pgvector::VectorExpressionMethods;
        use schema::document_chunks::{self, dsl};

        let chunks = document_chunks::table
            .filter(dsl::embedding.is_not_null())
            .order(dsl::embedding.cosine_distance(query_embedding))
            .limit(limit)
            .select(DocumentChunk::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(chunks)
    }

    async fn search_similar_chunks_in_files(
        &mut self,
        query_embedding: Vector,
        file_ids: &[Uuid],
        limit: i64,
    ) -> PgResult<Vec<DocumentChunk>> {
        use pgvector::VectorExpressionMethods;
        use schema::document_chunks::{self, dsl};

        if file_ids.is_empty() {
            return Ok(vec![]);
        }

        let chunks = document_chunks::table
            .filter(dsl::file_id.eq_any(file_ids))
            .filter(dsl::embedding.is_not_null())
            .order(dsl::embedding.cosine_distance(query_embedding))
            .limit(limit)
            .select(DocumentChunk::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(chunks)
    }

    async fn get_file_chunk_count(&mut self, file_id: Uuid) -> PgResult<i64> {
        use schema::document_chunks::{self, dsl};

        let count: i64 = document_chunks::table
            .filter(dsl::file_id.eq(file_id))
            .count()
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(count)
    }
}
