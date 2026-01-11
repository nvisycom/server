//! Document chunks repository for managing document text segments and embeddings.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use pgvector::Vector;
use uuid::Uuid;

use crate::model::{DocumentChunk, NewDocumentChunk, ScoredDocumentChunk, UpdateDocumentChunk};
use crate::{PgConnection, PgError, PgResult, schema};

/// Repository for document chunk database operations.
///
/// Handles chunk lifecycle management including creation, embedding updates,
/// and semantic similarity search via pgvector.
pub trait DocumentChunkRepository {
    /// Creates multiple document chunks in a single transaction.
    fn create_document_chunks(
        &mut self,
        new_chunks: Vec<NewDocumentChunk>,
    ) -> impl Future<Output = PgResult<Vec<DocumentChunk>>> + Send;

    /// Updates a chunk with new data.
    fn update_document_chunk(
        &mut self,
        chunk_id: Uuid,
        updates: UpdateDocumentChunk,
    ) -> impl Future<Output = PgResult<DocumentChunk>> + Send;

    /// Deletes all chunks for a file.
    fn delete_document_file_chunks(
        &mut self,
        file_id: Uuid,
    ) -> impl Future<Output = PgResult<usize>> + Send;

    /// Deletes all chunks for all files of a document.
    fn delete_document_chunks(
        &mut self,
        document_id: Uuid,
    ) -> impl Future<Output = PgResult<usize>> + Send;

    /// Lists all chunks for a specific file ordered by chunk index.
    fn list_document_file_chunks(
        &mut self,
        file_id: Uuid,
    ) -> impl Future<Output = PgResult<Vec<DocumentChunk>>> + Send;

    /// Searches for similar chunks using cosine similarity.
    ///
    /// Returns chunks ordered by similarity (most similar first).
    fn search_similar_document_chunks(
        &mut self,
        query_embedding: Vector,
        limit: i64,
    ) -> impl Future<Output = PgResult<Vec<DocumentChunk>>> + Send;

    /// Searches for similar chunks within specific files.
    fn search_similar_document_chunks_in_files(
        &mut self,
        query_embedding: Vector,
        file_ids: &[Uuid],
        limit: i64,
    ) -> impl Future<Output = PgResult<Vec<DocumentChunk>>> + Send;

    /// Searches for similar chunks within all files of specific documents.
    fn search_similar_document_chunks_in_documents(
        &mut self,
        query_embedding: Vector,
        document_ids: &[Uuid],
        limit: i64,
    ) -> impl Future<Output = PgResult<Vec<DocumentChunk>>> + Send;

    /// Searches for similar chunks within specific files with score filtering.
    ///
    /// Returns chunks with similarity score >= min_score, ordered by similarity.
    fn search_scored_chunks_in_files(
        &mut self,
        query_embedding: Vector,
        file_ids: &[Uuid],
        min_score: f64,
        limit: i64,
    ) -> impl Future<Output = PgResult<Vec<ScoredDocumentChunk>>> + Send;

    /// Searches for similar chunks within all files of specific documents with score filtering.
    ///
    /// Returns chunks with similarity score >= min_score, ordered by similarity.
    fn search_scored_chunks_in_documents(
        &mut self,
        query_embedding: Vector,
        document_ids: &[Uuid],
        min_score: f64,
        limit: i64,
    ) -> impl Future<Output = PgResult<Vec<ScoredDocumentChunk>>> + Send;

    /// Gets the total chunk count for a file.
    fn count_document_file_chunks(
        &mut self,
        file_id: Uuid,
    ) -> impl Future<Output = PgResult<i64>> + Send;
}

impl DocumentChunkRepository for PgConnection {
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

    async fn delete_document_file_chunks(&mut self, file_id: Uuid) -> PgResult<usize> {
        use schema::document_chunks::{self, dsl};

        let affected = diesel::delete(document_chunks::table.filter(dsl::file_id.eq(file_id)))
            .execute(self)
            .await
            .map_err(PgError::from)?;

        Ok(affected)
    }

    async fn delete_document_chunks(&mut self, document_id: Uuid) -> PgResult<usize> {
        use schema::document_chunks::{self, dsl};
        use schema::document_files;

        // Get all file IDs for this document
        let file_ids: Vec<Uuid> = document_files::table
            .filter(document_files::document_id.eq(document_id))
            .select(document_files::id)
            .load(self)
            .await
            .map_err(PgError::from)?;

        if file_ids.is_empty() {
            return Ok(0);
        }

        // Delete all chunks for those files
        let affected = diesel::delete(document_chunks::table.filter(dsl::file_id.eq_any(file_ids)))
            .execute(self)
            .await
            .map_err(PgError::from)?;

        Ok(affected)
    }

    async fn list_document_file_chunks(&mut self, file_id: Uuid) -> PgResult<Vec<DocumentChunk>> {
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

    async fn search_similar_document_chunks(
        &mut self,
        query_embedding: Vector,
        limit: i64,
    ) -> PgResult<Vec<DocumentChunk>> {
        use pgvector::VectorExpressionMethods;
        use schema::document_chunks::{self, dsl};

        let chunks = document_chunks::table
            .order(dsl::embedding.cosine_distance(&query_embedding))
            .limit(limit)
            .select(DocumentChunk::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(chunks)
    }

    async fn search_similar_document_chunks_in_files(
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
            .order(dsl::embedding.cosine_distance(&query_embedding))
            .limit(limit)
            .select(DocumentChunk::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(chunks)
    }

    async fn search_similar_document_chunks_in_documents(
        &mut self,
        query_embedding: Vector,
        document_ids: &[Uuid],
        limit: i64,
    ) -> PgResult<Vec<DocumentChunk>> {
        use pgvector::VectorExpressionMethods;
        use schema::document_chunks::{self, dsl};
        use schema::document_files;

        if document_ids.is_empty() {
            return Ok(vec![]);
        }

        // Get all file IDs for the given documents
        let file_ids: Vec<Uuid> = document_files::table
            .filter(document_files::document_id.eq_any(document_ids))
            .select(document_files::id)
            .load(self)
            .await
            .map_err(PgError::from)?;

        if file_ids.is_empty() {
            return Ok(vec![]);
        }

        let chunks = document_chunks::table
            .filter(dsl::file_id.eq_any(file_ids))
            .order(dsl::embedding.cosine_distance(&query_embedding))
            .limit(limit)
            .select(DocumentChunk::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(chunks)
    }

    async fn search_scored_chunks_in_files(
        &mut self,
        query_embedding: Vector,
        file_ids: &[Uuid],
        min_score: f64,
        limit: i64,
    ) -> PgResult<Vec<ScoredDocumentChunk>> {
        use pgvector::VectorExpressionMethods;
        use schema::document_chunks::{self, dsl};

        if file_ids.is_empty() {
            return Ok(vec![]);
        }

        // Cosine distance ranges from 0 (identical) to 2 (opposite)
        // Score = 1 - distance, so min_score threshold means max_distance = 1 - min_score
        let max_distance = 1.0 - min_score;

        let chunks: Vec<(DocumentChunk, f64)> = document_chunks::table
            .filter(dsl::file_id.eq_any(file_ids))
            .filter(
                dsl::embedding
                    .cosine_distance(&query_embedding)
                    .le(max_distance),
            )
            .order(dsl::embedding.cosine_distance(&query_embedding))
            .limit(limit)
            .select((
                DocumentChunk::as_select(),
                (1.0.into_sql::<diesel::sql_types::Double>()
                    - dsl::embedding.cosine_distance(&query_embedding)),
            ))
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(chunks
            .into_iter()
            .map(|(chunk, score)| ScoredDocumentChunk { chunk, score })
            .collect())
    }

    async fn search_scored_chunks_in_documents(
        &mut self,
        query_embedding: Vector,
        document_ids: &[Uuid],
        min_score: f64,
        limit: i64,
    ) -> PgResult<Vec<ScoredDocumentChunk>> {
        use pgvector::VectorExpressionMethods;
        use schema::document_chunks::{self, dsl};
        use schema::document_files;

        if document_ids.is_empty() {
            return Ok(vec![]);
        }

        // Get all file IDs for the given documents
        let file_ids: Vec<Uuid> = document_files::table
            .filter(document_files::document_id.eq_any(document_ids))
            .select(document_files::id)
            .load(self)
            .await
            .map_err(PgError::from)?;

        if file_ids.is_empty() {
            return Ok(vec![]);
        }

        let max_distance = 1.0 - min_score;

        let chunks: Vec<(DocumentChunk, f64)> = document_chunks::table
            .filter(dsl::file_id.eq_any(file_ids))
            .filter(
                dsl::embedding
                    .cosine_distance(&query_embedding)
                    .le(max_distance),
            )
            .order(dsl::embedding.cosine_distance(&query_embedding))
            .limit(limit)
            .select((
                DocumentChunk::as_select(),
                (1.0.into_sql::<diesel::sql_types::Double>()
                    - dsl::embedding.cosine_distance(&query_embedding)),
            ))
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(chunks
            .into_iter()
            .map(|(chunk, score)| ScoredDocumentChunk { chunk, score })
            .collect())
    }

    async fn count_document_file_chunks(&mut self, file_id: Uuid) -> PgResult<i64> {
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
